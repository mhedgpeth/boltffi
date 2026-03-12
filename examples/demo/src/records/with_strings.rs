use boltffi::*;

/// Represents a person with a name and age.
#[data]
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Person {
    pub name: String,
    pub age: u32,
}

#[export]
pub fn echo_person(p: Person) -> Person {
    p
}

#[export]
pub fn make_person(name: String, age: u32) -> Person {
    Person { name, age }
}

#[export]
pub fn greet_person(p: Person) -> String {
    format!("Hello, {}! You are {} years old.", p.name, p.age)
}

#[data]
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Address {
    pub street: String,
    pub city: String,
    pub zip: String,
}

#[export]
pub fn echo_address(a: Address) -> Address {
    a
}

#[export]
pub fn format_address(a: Address) -> String {
    format!("{}, {}, {}", a.street, a.city, a.zip)
}
