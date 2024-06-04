use leptos_struct_table::{ColumnSort, TableClassesProvider};

#[derive(Clone, Copy)]
pub struct ClassesPreset;

impl TableClassesProvider for ClassesPreset {
    fn new() -> Self {
        Self
    }

    fn thead_row(&self, template_classes: &str) -> String {
        format!(
            "{} {}",
            "text-xs text-gray-700 uppercase dark:text-gray-300", template_classes
        )
    }

    fn thead_cell(&self, sort: ColumnSort, template_classes: &str) -> String {
        let sort_class = match sort {
            ColumnSort::None => "",
            _ => "text-black dark:text-white",
        };

        format!(
            "bg-zinc-200 dark:bg-zinc-700 cursor-pointer px-5 py-2 sticky top-0 whitespace-nowrap {} {}",
            sort_class, template_classes
        )
    }

    fn thead_cell_inner(&self) -> String {
        "flex items-center after:content-[--sort-icon] after:pl-1 after:opacity-40 before:content-[--sort-priority] before:order-last before:pl-0.5 before:font-light before:opacity-40".to_string()
    }

    fn row(&self, row_index: usize, selected: bool, template_classes: &str) -> String {
        let bg_color = if row_index % 2 == 0 {
            if selected {
                "bg-sky-300 text-gray-700 dark:bg-sky-700 dark:text-gray-400"
            } else {
                "bg-white dark:bg-zinc-900 hover:bg-zinc-100 dark:hover:bg-zinc-600"
            }
        } else if selected {
            "bg-sky-300 text-gray-700 dark:bg-sky-700 dark:text-gray-400"
        } else {
            "bg-zinc-50 dark:bg-zinc-800 hover:bg-zinc-100 dark:hover:bg-zinc-600"
        };

        format!(
            "{} {} {}",
            "border-b dark:border-gray-700", bg_color, template_classes
        )
    }

    fn loading_cell(&self, _row_index: usize, _col_index: usize, prop_class: &str) -> String {
        format!("{} {}", "px-5 py-2", prop_class)
    }

    fn loading_cell_inner(&self, row_index: usize, _col_index: usize, prop_class: &str) -> String {
        let width = match row_index % 4 {
            0 => "w-[calc(85%-2.5rem)]",
            1 => "w-[calc(90%-2.5rem)]",
            2 => "w-[calc(75%-2.5rem)]",
            _ => "w-[calc(60%-2.5rem)]",
        };
        format!(
            "animate-pulse h-2 bg-zinc-200 rounded-full dark:bg-zinc-700 inline-block align-middle {} {}",
            width, prop_class
        )
    }

    fn cell(&self, template_classes: &str) -> String {
        format!(
            "{} {}",
            "px-5 py-2 whitespace-nowrap overflow-hidden text-ellipsis", template_classes
        )
    }
}
