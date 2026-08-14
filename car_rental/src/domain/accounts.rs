#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UserRole {
    Customer,
    Receptionist,
    Manager,
}

#[derive(Clone, Debug)]
pub struct User {
    pub id: u64,
    pub name: String,
    pub role: UserRole,
}

impl User {
    pub fn new(id: u64, name: &str, role: UserRole) -> Self {
        Self {
            id,
            name: name.to_string(),
            role,
        }
    }

    pub fn is_manager(&self) -> bool {
        self.role == UserRole::Manager
    }

    pub fn is_staff_or_manager(&self) -> bool {
        matches!(self.role, UserRole::Receptionist | UserRole::Manager)
    }
}

#[derive(Clone, Debug)]
pub struct Customer {
    pub id: u64,
    pub name: String,
    pub email: String,
    pub phone: String,
    pub license_number: String,
}

#[derive(Clone, Debug)]
pub struct Address {
    pub street: String,
    pub city: String,
    pub state: String,
    pub zip_code: String,
    pub country: String,
}
