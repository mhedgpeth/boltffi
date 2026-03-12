use boltffi::*;

/// A user profile where some fields may not be set.
#[data]
#[derive(Clone, Debug, PartialEq, Default)]
pub struct UserProfile {
    pub name: String,
    pub age: u32,
    /// Contact email, if the user has provided one.
    pub email: Option<String>,
    /// Reputation score, absent for new users.
    pub score: Option<f64>,
}

#[export]
pub fn echo_user_profile(profile: UserProfile) -> UserProfile {
    profile
}

#[export]
pub fn make_user_profile(
    name: String,
    age: u32,
    email: Option<String>,
    score: Option<f64>,
) -> UserProfile {
    UserProfile {
        name,
        age,
        email,
        score,
    }
}

#[export]
pub fn user_display_name(profile: UserProfile) -> String {
    match profile.email {
        Some(email) => format!("{} <{}>", profile.name, email),
        None => profile.name,
    }
}

#[data]
#[derive(Clone, Debug, PartialEq, Default)]
pub struct SearchResult {
    pub query: String,
    pub total: u32,
    pub next_cursor: Option<String>,
    pub max_score: Option<f64>,
}

#[export]
pub fn echo_search_result(result: SearchResult) -> SearchResult {
    result
}

#[export]
pub fn has_more_results(result: SearchResult) -> bool {
    result.next_cursor.is_some()
}
