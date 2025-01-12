pub mod settings;

pub struct FromTestEntity {}
pub struct FooEntity;
pub struct FooModel;

pub trait FooEntityTrait {
    type Model;
}

impl FooEntityTrait for FooEntity {
    type Model = FooModel;
}

impl std::convert::From<<FooEntity as FooEntityTrait>::Model> for FromTestEntity {
    fn from(_m: <FooEntity as FooEntityTrait>::Model) -> Self {
        Self {}
    }
}
