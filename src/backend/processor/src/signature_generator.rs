use fancy_regex::Regex;
use serde::Deserialize;

use crate::error::JobError;
use crate::utils::JsonHelpers;

#[derive(Debug, Deserialize)]
pub struct SignatureGeneratorConfig {
    pub skip_patterns: Vec<String>,
    pub end_patterns: Vec<String>,
    pub delimiter: String,
    pub maximum_frame_count: usize,
}

impl Default for SignatureGeneratorConfig {
    fn default() -> Self {
        Self {
            skip_patterns: vec![],
            end_patterns: vec![],
            delimiter: "|".into(),
            maximum_frame_count: 10,
        }
    }
}

impl SignatureGeneratorConfig {
    pub fn new(skip_patterns: Vec<String>, end_patterns: Vec<String>) -> Self {
        Self {
            skip_patterns,
            end_patterns,
            delimiter: "|".into(),
            maximum_frame_count: 10,
        }
    }
}

pub struct SignatureGenerator {
    skip_patterns_regex: Regex,
    has_skip_patterns: bool,
    end_patterns_regex: Regex,
    has_end_patterns: bool,
    template_parameters_pattern_regex: Regex,
    delimiter: String,
    maximum_frame_count: usize,
}

impl SignatureGenerator {
    pub fn new(config: SignatureGeneratorConfig) -> Result<Self, JobError> {
        let skip_patterns_regex = Self::build_regex(&config.skip_patterns)?;
        let end_patterns_regex = Self::build_regex(&config.end_patterns)?;

        let template_parameters_pattern = r"<[^<>]*(?:<[^<>]*>[^<>]*)*>";
        let template_parameters_pattern_regex = Regex::new(template_parameters_pattern)?;

        Ok(Self {
            skip_patterns_regex,
            has_skip_patterns: !config.skip_patterns.is_empty(),
            end_patterns_regex,
            has_end_patterns: !config.end_patterns.is_empty(),
            template_parameters_pattern_regex,
            delimiter: config.delimiter,
            maximum_frame_count: config.maximum_frame_count,
        })
    }

    fn build_regex(patterns: &[String]) -> Result<Regex, JobError> {
        let joined = patterns.join("|");
        Ok(Regex::new(&joined)?)
    }

    fn extract_function_name(signature: &str) -> String {
        let trimmed = signature.trim();

        // Find the last closing parenthesis (end of parameter list)
        let last_close_paren = match trimmed.rfind(')') {
            Some(pos) => pos,
            None => return trimmed.to_owned(), // No parentheses at all
        };

        // Search backward from the closing paren to find the matching opening paren
        let mut paren_depth = 0;
        let mut param_start = None;

        for (i, ch) in trimmed[..=last_close_paren].char_indices().rev() {
            match ch {
                ')' => paren_depth += 1,
                '(' => {
                    paren_depth -= 1;
                    if paren_depth == 0 {
                        param_start = Some(i);
                        break;
                    }
                }
                _ => {}
            }
        }

        let paren_pos = match param_start {
            Some(pos) => pos,
            None => return trimmed.to_owned(), // Unmatched parentheses
        };

        // Find the start of the function name by walking backwards from the opening paren
        let mut in_anon_quote = false;
        let mut angle_depth = 0;
        let mut func_start = 0usize;

        // Determine search end point: for operators, search from before "operator" keyword
        let search_end = match trimmed[..paren_pos].rfind("operator") {
            Some(operator_pos) => operator_pos,
            None => paren_pos,
        };

        for (idx, ch) in trimmed[..search_end].char_indices().rev() {
            match ch {
                '\'' if !in_anon_quote => in_anon_quote = true,
                '`' if in_anon_quote => in_anon_quote = false,
                '>' if !in_anon_quote => angle_depth += 1,
                '<' if !in_anon_quote => angle_depth -= 1,
                c if c.is_whitespace() && !in_anon_quote && angle_depth == 0 => {
                    func_start = idx + 1;
                    break;
                }
                _ => {}
            }
        }

        // Return the function name without the parameter list
        trimmed[func_start..paren_pos].to_owned()
    }

    fn collapse_template_parameters(&self, function: &str) -> String {
        self.template_parameters_pattern_regex
            .replace_all(function, "<T>")
            .to_string()
    }

    fn condense_function_name(&self, function: &str) -> String {
        let function = Self::extract_function_name(function);
        self.collapse_template_parameters(&function)
    }

    fn get_module_prefix_from_frame(frame: &serde_json::Value) -> String {
        let module = JsonHelpers::get_string(frame, "module").unwrap_or_default();
        let module = module.split(['/', '\\']).next_back().unwrap_or_default();

        if module.is_empty() {
            String::new()
        } else {
            format!("{module}!")
        }
    }

    fn generate_frame_signature(&self, frame: &serde_json::Value) -> String {
        let module_prefix = Self::get_module_prefix_from_frame(frame);

        if let Some(function) = JsonHelpers::get_string(frame, "function")
            && !function.is_empty()
        {
            let func = self.condense_function_name(&function);
            return format!("{module_prefix}{func}");
        }

        if let (Some(file), Some(line)) =
            (JsonHelpers::get_string(frame, "file"), JsonHelpers::get_u32(frame, "line"))
            && !file.is_empty()
        {
            let file = file.split(['/', '\\']).next_back().unwrap_or(&file);
            return format!("{module_prefix}{file}#{line}");
        }

        if !module_prefix.is_empty() {
            return module_prefix.to_string();
        }

        String::new()
    }

    fn generate_signatures(&self, frames: Vec<serde_json::Value>) -> Vec<String> {
        let mut signatures = Vec::new();

        for frame in frames {
            let signature = self.generate_frame_signature(&frame);
            signatures.push(signature);
        }

        signatures
    }

    fn transfer_missing_field(
        child_frame: &mut serde_json::Value,
        parent_frame: &serde_json::Value,
        field_name: &str,
    ) {
        if JsonHelpers::get_string(child_frame, field_name).is_none()
            && let Some(value) = JsonHelpers::get_string(parent_frame, field_name)
        {
            child_frame[field_name] = serde_json::Value::String(value);
        }
    }

    fn flatten_frame_list(&self, crashing_thread: &serde_json::Value) -> Vec<serde_json::Value> {
        let mut flattened_frame_list = Vec::new();
        let frame_list = JsonHelpers::get_array(crashing_thread, "frames");

        for frame in frame_list {
            let inline_frames = JsonHelpers::get_array(frame, "inlines");
            for inline_frame in inline_frames {
                let mut new_frame = inline_frame.clone();
                Self::transfer_missing_field(&mut new_frame, frame, "module");
                Self::transfer_missing_field(&mut new_frame, frame, "module_offset");
                Self::transfer_missing_field(&mut new_frame, frame, "offset");
                flattened_frame_list.push(new_frame);
            }
            flattened_frame_list.push(frame.clone());
        }
        flattened_frame_list
    }

    pub fn generate(&self, crashing_thread: &serde_json::Value) -> Result<String, JobError> {
        let frames = self.flatten_frame_list(crashing_thread);
        let signatures = self.generate_signatures(frames);

        let mut relevant_signatures = Vec::new();
        for signature in signatures.iter() {
            if signature.is_empty() {
                continue;
            }

            if self.has_skip_patterns && self.skip_patterns_regex.is_match(signature)? {
                continue;
            }

            relevant_signatures.push(signature.clone());

            if self.has_end_patterns && self.end_patterns_regex.is_match(signature)? {
                break;
            }
        }

        if relevant_signatures.len() > self.maximum_frame_count {
            relevant_signatures.truncate(self.maximum_frame_count);
        }

        let mut signature = relevant_signatures.join(&self.delimiter);

        if signature.is_empty() {
            signature = "NONE".to_string();
        }

        Ok(signature)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_frame() {
        let config = SignatureGeneratorConfig::default();
        let generator = SignatureGenerator::new(config).unwrap();

        let frame = serde_json::json!({
            "function": "void foo() const",
            "module": "test.dll",
            "file": "test.cpp",
            "line": 42
        });
        let normalized = generator.generate_frame_signature(&frame);
        assert_eq!(normalized, "test.dll!foo");

        let frame = serde_json::json!({
            "module": "test.dll",
            "file": "test.cpp",
            "line": 42
        });
        let normalized = generator.generate_frame_signature(&frame);
        assert_eq!(normalized, "test.dll!test.cpp#42");

        let frame = serde_json::json!({
            "module": "a/b/c/foo/test.dll",
            "file": "test.cpp",
            "line": 42
        });
        let normalized = generator.generate_frame_signature(&frame);
        assert_eq!(normalized, "test.dll!test.cpp#42");

        let frame = serde_json::json!({
            "module": "a\\b\\c\\foo\\test.dll",
            "file": "test.cpp",
            "line": 42
        });
        let normalized = generator.generate_frame_signature(&frame);
        assert_eq!(normalized, "test.dll!test.cpp#42");

        let frame = serde_json::json!({
            "module": "test.dll",
            "offset": "0x1234"
        });
        let normalized = generator.generate_frame_signature(&frame);
        assert_eq!(normalized, "test.dll!");
    }

    #[test]
    fn test_inline_frames() {
        let config = SignatureGeneratorConfig::default();
        let generator = SignatureGenerator::new(config).unwrap();

        let thread_data = serde_json::json!({
            "frames": [
                {
                    "function": "main_function",
                    "module": "main.dll",
                    "inlines": [
                        {
                            "function": "inline_func1",
                            "file": "inline.h",
                            "line": 10
                        },
                        {
                            "function": "inline_func2",
                            "file": "inline.h",
                            "line": 20
                        }
                    ]
                }
            ]
        });

        let flattened_frames = generator.flatten_frame_list(&thread_data);
        let frame_list = generator.generate_signatures(flattened_frames);

        assert_eq!(frame_list.len(), 3);
        assert!(frame_list.iter().any(|f| f.contains("inline_func1")));
        assert!(frame_list.iter().any(|f| f.contains("inline_func2")));
        assert!(frame_list.iter().any(|f| f.contains("main_function")));
    }

    #[test]
    fn test_generate_empty_signature() {
        let config = SignatureGeneratorConfig::default();
        let generator = SignatureGenerator::new(config).unwrap();

        let thread_data = serde_json::json!({"frames": []});
        let result = generator.generate(&thread_data).unwrap();

        assert!(result.starts_with("NONE"));
    }

    #[test]
    fn test_strip_function_name() {
        let input = "void `anonymous namespace'::MyClass::function()";
        let expected = "`anonymous namespace'::MyClass::function";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected.to_string());

        let input = "int foo(int)";
        let expected = "foo";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected.to_string());

        let input = "inline constexpr std::vector<int> MyNS::util::baz<std::string>(std::string)";
        let expected = "MyNS::util::baz<std::string>";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected.to_string());

        let input = "static void * Allocator<MyClass>::function(unsigned __int64)";
        let expected = "Allocator<MyClass>::function";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);

        let input = "int MyClass::myFunction()";
        let expected = "MyClass::myFunction";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);

        let input = "const int MyClass::myFunction()";
        let expected = "MyClass::myFunction";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);

        let input = "static void MyClass::staticFunction()";
        let expected = "MyClass::staticFunction";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);

        let input = "static const volatile unsigned long long MyClass::complexFunction()";
        let expected = "MyClass::complexFunction";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);

        let input = "std::vector<int> MyTemplate<T>::getVector()";
        let expected = "MyTemplate<T>::getVector";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);

        let input = "char* MyClass::getString()";
        let expected = "MyClass::getString";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);

        let input = "const std::string& MyClass::getReference()";
        let expected = "MyClass::getReference";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);

        let input = "std::shared_ptr<std::vector<int>> MyClass::getComplexType()";
        let expected = "MyClass::getComplexType";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);

        let input = "void myns::MyClass::myFunction()";
        let expected = "myns::MyClass::myFunction";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);

        let input = "int ns1::ns2::ns3::MyClass::deepFunction()";
        let expected = "ns1::ns2::ns3::MyClass::deepFunction";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);

        let input = "bool MyClass::compare(const std::string& a, const std::string& b)";
        let expected = "MyClass::compare";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);

        let input = "MyClass::MyClass(int value)";
        let expected = "MyClass::MyClass";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);

        let input = "MyClass::~MyClass()";
        let expected = "MyClass::~MyClass";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);

        let input = "bool MyClass::operator==(const MyClass& other)";
        let expected = "MyClass::operator==";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);

        let input = "template<typename T> void MyClass::templateFunction()";
        let expected = "MyClass::templateFunction";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);

        let input = "`anonymous namespace'::function()";
        let expected = "`anonymous namespace'::function";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);

        let input = "void `anonymous namespace'::MyClass::function()";
        let expected = "`anonymous namespace'::MyClass::function";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);

        let input = "int globalFunction()";
        let expected = "globalFunction";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);

        let input = "auto MyClass::modernFunction()";
        let expected = "MyClass::modernFunction";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);

        let input = "auto MyClass::trailingReturn() -> int";
        let expected = "MyClass::trailingReturn";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);

        let input = "";
        let expected = "";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);

        let input = "function";
        let expected = "function";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);

        let input = "virtual void MyClass::virtualFunction()";
        let expected = "MyClass::virtualFunction";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);

        let input = "virtual void MyClass::pureVirtual() = 0";
        let expected = "MyClass::pureVirtual";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);

        let input = "void MyClass::noThrowFunction() noexcept";
        let expected = "MyClass::noThrowFunction";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);

        let input = "static const volatile std::shared_ptr<std::vector<MyTemplate<int>>> ns::MyClass<T>::complexMethod(const Args&... args) noexcept";
        let expected = "ns::MyClass<T>::complexMethod";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);

        let input = "MyClass::operator bool() const";
        let expected = "MyClass::operator bool";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);

        let input = "operator delete(void* ptr)";
        let expected = "operator delete";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);

        let input = "static void* operator new(size_t size)";
        let expected = "operator new";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);

        let input = "const MyClass& MyClass::operator=(const MyClass& other)";
        let expected = "MyClass::operator=";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);

        let input =
            "virtual bool MyNamespace::MyClass::operator!=(const MyClass& other) const noexcept";
        let expected = "MyNamespace::MyClass::operator!=";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);

        let input = "operator new[](size_t size)";
        let expected = "operator new[]";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);

        let input = "std::map<std::string, std::vector<int>>::operator[](const std::string& key)";
        let expected = "std::map<std::string, std::vector<int>>::operator[]";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);
    }

    #[test]
    fn test_strip_function_name_conversion_operators() {
        let input = "MyClass::operator bool() const";
        let expected = "MyClass::operator bool";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);

        let input = "operator int() const";
        let expected = "operator int";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);

        let input = "MyClass::operator const char*() const";
        let expected = "MyClass::operator const char*";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);

        let input = "MyClass::operator std::string() const";
        let expected = "MyClass::operator std::string";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);

        let input = "operator std::vector<int>() const";
        let expected = "operator std::vector<int>";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);

        let input = "MyClass::operator MyCustomType() const";
        let expected = "MyClass::operator MyCustomType";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);

        let input = "operator decltype(auto)() const";
        let expected = "operator decltype(auto)";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);

        let input = "operator const volatile unsigned long() const";
        let expected = "operator const volatile unsigned long";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);

        let input = "operator unsigned long long() const";
        let expected = "operator unsigned long long";
        assert_eq!(SignatureGenerator::extract_function_name(input), expected);
    }

    #[test]
    fn test_normalize_cpp_function() {
        let config = SignatureGeneratorConfig::default();
        let generator = SignatureGenerator::new(config).unwrap();

        let result = generator.condense_function_name("void foo() const");
        assert_eq!(result, "foo");

        let result = generator.condense_function_name("int MyClass::getValue() const");
        assert_eq!(result, "MyClass::getValue");

        let result = generator.condense_function_name("static const void* MyClass::getData()");
        assert_eq!(result, "MyClass::getData");

        let result = generator.condense_function_name("std::vector<int> Container::getItems()");
        assert_eq!(result, "Container::getItems");

        let result = generator
            .condense_function_name("std::shared_ptr<std::vector<MyClass>> Factory::create()");
        assert_eq!(result, "Factory::create");

        let result =
            generator.condense_function_name("void MyClass::process(int x, char* y, bool z)");
        assert_eq!(result, "MyClass::process");

        let result =
            generator.condense_function_name("bool MyClass::operator==(const MyClass& other)");
        assert_eq!(result, "MyClass::operator==");

        let result =
            generator.condense_function_name("MyClass& MyClass::operator<(const MyClass& other)");
        assert_eq!(result, "MyClass::operator<");

        let result = generator.condense_function_name("`anonymous namespace'::helper()");
        assert_eq!(result, "`anonymous namespace'::helper");

        let result = generator.condense_function_name("`anonymous namespace'::func(int x, int y)");
        assert_eq!(result, "`anonymous namespace'::func");

        let result = generator.condense_function_name("MyClass::$_0::operator()");
        assert_eq!(result, "MyClass::$_0::operator");

        let result = generator.condense_function_name("MyClass::func()[clone .cold.123]");
        assert_eq!(result, "MyClass::func");

        let result = generator.condense_function_name("char * MyClass::getString()");
        assert_eq!(result, "MyClass::getString");

        let result = generator.condense_function_name("void func(int,char,bool)");
        assert_eq!(result, "func");

        let result = generator.condense_function_name(
            "static const std::unique_ptr<MyTemplate<int>> ns::MyClass::factory(const Args& args, bool flag)",
        );
        assert_eq!(result, "ns::MyClass::factory");

        let result = generator.condense_function_name("MyClass::MyClass(int value)");
        assert_eq!(result, "MyClass::MyClass");

        let result = generator.condense_function_name("MyClass::~MyClass()");
        assert_eq!(result, "MyClass::~MyClass");

        let result = generator.condense_function_name("virtual void MyClass::process()");
        assert_eq!(result, "MyClass::process");

        let result = generator.condense_function_name("inline int MyClass::getValue() const");
        assert_eq!(result, "MyClass::getValue");

        let result = generator.condense_function_name("void ns1::ns2::MyClass::func()");
        assert_eq!(result, "ns1::ns2::MyClass::func");

        let result = generator.condense_function_name("int globalFunction(int x, int y)");
        assert_eq!(result, "globalFunction");

        let result = generator.condense_function_name("auto MyClass::getValue() -> int");
        assert_eq!(result, "MyClass::getValue");

        let result = generator.condense_function_name("void MyClass::safeFunc() noexcept");
        assert_eq!(result, "MyClass::safeFunc");

        let result =
            generator.condense_function_name("template<typename T> void MyClass::process()");
        assert_eq!(result, "MyClass::process");

        let result = generator.condense_function_name("void* MyClass::operator new(size_t size)");
        assert_eq!(result, "MyClass::operator new");

        let result = generator.condense_function_name(
            "std::ostream& operator<<(std::ostream& os, const MyClass<T>& obj)",
        );
        assert_eq!(result, "operator<<");

        let result = generator.condense_function_name("void MyClass::log(const char* fmt, ...)");
        assert_eq!(result, "MyClass::log");

        let result = generator.condense_function_name("MyClass::$_1::operator()(int x)");
        assert_eq!(result, "MyClass::$_1::operator()");

        let result = generator.condense_function_name("");
        assert_eq!(result, "");

        let result = generator.condense_function_name("static inline constexpr");
        assert_eq!(result, "static inline constexpr");
    }

    #[test]
    fn test_normalize_cpp_function_crashdump_scenarios() {
        let config = SignatureGeneratorConfig::default();
        let generator = SignatureGenerator::new(config).unwrap();

        let result = generator.condense_function_name("MyClass::getCallback()");
        assert_eq!(result, "MyClass::getCallback");

        let result = generator.condense_function_name("MyClass::process(int x, char* data)");
        assert_eq!(result, "MyClass::process");

        let result =
            generator.condense_function_name("const std::string& MyClass::getName() const");
        assert_eq!(result, "MyClass::getName");

        let result = generator.condense_function_name("static void MyClass::cleanup()");
        assert_eq!(result, "MyClass::cleanup");

        let result = generator.condense_function_name("virtual void MyClass::process()");
        assert_eq!(result, "MyClass::process");

        let result = generator.condense_function_name("MyClass::~MyClass()");
        assert_eq!(result, "MyClass::~MyClass");

        let result = generator.condense_function_name("MyClass::MyClass(int value)");
        assert_eq!(result, "MyClass::MyClass");

        let vector_result =
            generator.condense_function_name("std::vector<int>::push_back(const int& value)");
        assert_eq!(vector_result, "std::vector<T>::push_back");

        let map_result = generator.condense_function_name(
            "std::map<std::string, std::vector<int>>::operator[](const std::string& key)",
        );
        assert_eq!(map_result, "std::map<T>::operator[]");

        let equals_result =
            generator.condense_function_name("bool MyClass::operator==(const MyClass& other)");
        assert_eq!(equals_result, "MyClass::operator==");

        let assign_result =
            generator.condense_function_name("MyClass& MyClass::operator=(const MyClass& rhs)");
        assert_eq!(assign_result, "MyClass::operator=");

        let subscript_result =
            generator.condense_function_name("T& MyClass::operator[](size_t index)");
        assert_eq!(subscript_result, "MyClass::operator[]");

        let call_result = generator.condense_function_name("int MyClass::operator()(int x, int y)");
        assert_eq!(call_result, "MyClass::operator()");

        let result = generator.condense_function_name("`anonymous namespace'::helper(int x)");
        assert_eq!(result, "`anonymous namespace'::helper");

        let result = generator
            .condense_function_name("`anonymous namespace'::`anonymous namespace'::utility()");
        assert_eq!(result, "`anonymous namespace'::`anonymous namespace'::utility");

        let result = generator.condense_function_name("MyClass::$_123::operator()(int x)");
        assert_eq!(result, "MyClass::$_123::operator()");

        let result = generator.condense_function_name("MyClass::processData()[clone .cold.1]");
        assert_eq!(result, "MyClass::processData");

        let result = generator.condense_function_name("globalFunction(int x, char* data)");
        assert_eq!(result, "globalFunction");

        let result = generator.condense_function_name("ns1::ns2::MyClass::method()");
        assert_eq!(result, "ns1::ns2::MyClass::method");

        let result = generator
            .condense_function_name("app::core::utils::StringHelper::trim(const std::string& str)");
        assert_eq!(result, "app::core::utils::StringHelper::trim");

        let result = generator.condense_function_name("std::terminate()");
        assert_eq!(result, "std::terminate");

        let result = generator.condense_function_name("operator new(size_t size)");
        assert_eq!(result, "operator new");

        let result = generator.condense_function_name("operator delete(void* ptr)");
        assert_eq!(result, "operator delete");

        let result = generator.condense_function_name("MyClass::operator bool() const");
        assert_eq!(result, "MyClass::operator bool");

        let result =
            generator.condense_function_name("operator<<(std::ostream& os, const MyClass& obj)");
        assert_eq!(result, "operator<<");

        let result = generator.condense_function_name("MyTemplate<int>::specializedMethod()");
        assert_eq!(result, "MyTemplate<T>::specializedMethod");

        let result =
            generator.condense_function_name("std::shared_ptr<std::vector<MyClass>>::reset()");
        assert_eq!(result, "std::shared_ptr<T>::reset");
    }

    #[test]
    fn test_normalize_cpp_function_mangled_names() {
        let config = SignatureGeneratorConfig::default();
        let generator = SignatureGenerator::new(config).unwrap();

        let result = generator.condense_function_name("_Z3fooi");
        assert_eq!(result, "_Z3fooi");

        let result = generator.condense_function_name("_ZN7MyClassC1Ei");
        assert_eq!(result, "_ZN7MyClassC1Ei");

        let result = generator.condense_function_name("_ZN7MyClassD1Ev");
        assert_eq!(result, "_ZN7MyClassD1Ev");

        let result = generator.condense_function_name("?function@@YAHH@Z");
        assert_eq!(result, "?function@@YAHH@Z");

        let result = generator.condense_function_name("std::vector<int>::push_back[abi:cxx11]()");
        assert_eq!(result, "std::vector<T>::push_back[abi:cxx11]");

        let result = generator.condense_function_name("std::string::c_str[abi:cxx11]() const");
        assert_eq!(result, "std::string::c_str[abi:cxx11]");

        let result =
            generator.condense_function_name("std::make_shared<MyClass>@@GLIBCXX_3.4.21()");
        assert_eq!(result, "std::make_shared<T>@@GLIBCXX_3.4.21");
    }

    #[test]
    fn test_normalize_cpp_function_unusual_syntax() {
        let config = SignatureGeneratorConfig::default();
        let generator = SignatureGenerator::new(config).unwrap();

        let result = generator.condense_function_name("void __stdcall MyClass::winApiMethod()");
        assert_eq!(result, "MyClass::winApiMethod");

        let result = generator.condense_function_name("int __fastcall MyClass::fastMethod(int x)");
        assert_eq!(result, "MyClass::fastMethod");

        let result = generator.condense_function_name("void __vectorcall MyClass::vectorMethod()");
        assert_eq!(result, "MyClass::vectorMethod");

        let result = generator.condense_function_name("MyClass MyClass::copy() const &");
        assert_eq!(result, "MyClass::copy");

        let result = generator.condense_function_name("MyClass MyClass::move() &&");
        assert_eq!(result, "MyClass::move");

        let result = generator
            .condense_function_name("void MyClass::complexMethod() const volatile & noexcept");
        assert_eq!(result, "MyClass::complexMethod");

        let result =
            generator.condense_function_name("void MyClass::virtualMethod() override final");
        assert_eq!(result, "MyClass::virtualMethod");
    }

    #[test]
    fn test_generate_with_multiple_frames() {
        let config = SignatureGeneratorConfig::default();
        let generator = SignatureGenerator::new(config).unwrap();

        let thread_data = serde_json::json!({
            "frames": [
                {
                    "module": "crash.exe",
                    "function": "main",
                    "line": 42,
                    "file": "main.cpp"
                },
                {
                    "module": "crash.exe",
                    "function": "processData",
                    "line": 15,
                    "file": "processor.cpp"
                },
                {
                    "module": "helper.dll",
                    "function": "calculateResult",
                    "line": 89,
                    "file": "math.cpp"
                }
            ]
        });

        let result = generator.generate(&thread_data).unwrap();
        assert_eq!(result, "crash.exe!main|crash.exe!processData|helper.dll!calculateResult");
    }

    #[test]
    fn test_generate_with_inline_frames() {
        let config = SignatureGeneratorConfig::default();
        let generator = SignatureGenerator::new(config).unwrap();

        let thread_data = serde_json::json!({
            "frames": [
                {
                    "module": "crash.exe",
                    "function": "outerFunction",
                    "line": 100,
                    "inlines": [
                        {"module": "crash.exe", "function": "inlineHelper1", "line": 50},
                        {"module": "crash.exe", "function": "inlineHelper2", "line": 75}
                    ]
                },
                {
                    "module": "crash.exe",
                    "function": "mainFunction",
                    "line": 200
                }
            ]
        });

        let result = generator.generate(&thread_data).unwrap();
        assert_eq!(
            result,
            "crash.exe!inlineHelper1|crash.exe!inlineHelper2|crash.exe!outerFunction|crash.exe!mainFunction"
        );
    }

    #[test]
    fn test_generate_with_skip_patterns() {
        let config = SignatureGeneratorConfig::new(
            vec![".*test.*".to_string(), ".*debug.*".to_string()],
            vec![],
        );
        let generator = SignatureGenerator::new(config).unwrap();

        let thread_data = serde_json::json!({
            "frames": [
                {"module" : "crash.exe", "function": "testFunction", "line": 1},
                {"module" : "crash.exe", "function": "debugHelper", "line": 2},
                {"module" : "crash.exe", "function": "actualFunction", "line": 3},
                {"module" : "crash.exe", "function": "importantFunction", "line": 4}
            ]
        });

        let result = generator.generate(&thread_data).unwrap();
        assert_eq!(result, "crash.exe!actualFunction|crash.exe!importantFunction");
    }

    #[test]
    fn test_generate_with_end_patterns() {
        let config =
            SignatureGeneratorConfig::new(vec![], vec!["main".to_string(), "entry.*".to_string()]);
        let generator = SignatureGenerator::new(config).unwrap();

        let thread_data = serde_json::json!({
            "frames": [
                {"module" : "crash.exe", "function": "specificFunction", "line": 1},
                {"module" : "crash.exe", "function": "main", "line": 2},
                {"module" : "crash.exe", "function": "shouldNotAppear", "line": 3},
                {"module" : "crash.exe", "function": "alsoIgnored", "line": 4}
            ]
        });

        let result = generator.generate(&thread_data).unwrap();
        assert_eq!(result, "crash.exe!specificFunction|crash.exe!main");
    }

    #[test]
    fn test_generate_with_complex_cpp_functions() {
        let config = SignatureGeneratorConfig::default();
        let generator = SignatureGenerator::new(config).unwrap();

        let thread_data = serde_json::json!({
            "frames": [
                {
                    "module": "crash.exe",
                    "function": "std::vector<std::string>::push_back(const std::string&)",
                    "line": 1
                },
                {
                    "module": "crash.exe",
                    "function": "MyNamespace::MyClass::processData(int, bool) const",
                    "line": 2
                },
                {
                    "module": "helper.dll",
                    "function": "operator<<(std::ostream&, const MyClass&)",
                    "line": 3
                }
            ]
        });

        let result = generator.generate(&thread_data).unwrap();
        assert_eq!(
            result,
            "crash.exe!std::vector<T>::push_back|crash.exe!MyNamespace::MyClass::processData|helper.dll!operator<<"
        );
    }

    #[test]
    fn test_generate_mixed_scenarios() {
        let config = SignatureGeneratorConfig::default();
        let generator = SignatureGenerator::new(config).unwrap();

        let thread_data = serde_json::json!({
            "frames": [
                {"function": "", "line": 1},  // Empty function
                {"line": 2},  // Missing function
                {"module" : "crash.exe", "function": "validFunction", "line": 3},
                {
                    "function": "frameWithInlines",
                    "line": 4,
                    "inlines": [
                        {"function": "", "line": 10},  // Empty inline
                        {"function": "validInline", "line": 11}
                    ]
                },
                {"module" : "crash.exe", "function": "finalFunction", "line": 5}
            ]
        });

        let result = generator.generate(&thread_data).unwrap();
        assert_eq!(
            result,
            "crash.exe!validFunction|validInline|frameWithInlines|crash.exe!finalFunction"
        );
    }
}
