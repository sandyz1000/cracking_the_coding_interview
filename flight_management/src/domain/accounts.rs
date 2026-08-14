#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UserRole {
    Passenger,
    Staff,
    Admin,
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

    pub fn is_admin(&self) -> bool {
        self.role == UserRole::Admin
    }

    pub fn is_staff_or_admin(&self) -> bool {
        matches!(self.role, UserRole::Staff | UserRole::Admin)
    }
}

#[derive(Clone, Debug)]
pub struct Passenger {
    pub id: u64,
    pub name: String,
    pub email: String,
    pub phone: String,
}
