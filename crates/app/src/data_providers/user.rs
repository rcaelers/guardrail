use ::chrono::NaiveDateTime;
use cfg_if::cfg_if;
use leptos::*;
use leptos_struct_table::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

cfg_if! { if #[cfg(feature="ssr")] {
    use sea_orm::*;
    use sea_query::Expr;
    use std::collections::HashMap;
    use crate::entity;
    use crate::auth::AuthenticatedUser;
    use crate::data::{
        add, count, delete_by_id, get_all, get_all_names, get_by_id, update, EntityInfo,
    };
}}

use super::ExtraRowTrait;
use crate::classes::ClassesPreset;
use crate::data::QueryParams;

#[derive(TableRow, Debug, Clone)]
#[table(sortable, classes_provider = ClassesPreset)]
pub struct UserRow {
    pub id: Uuid,
    pub username: String,
    pub is_admin: bool,
    #[table(format(string = "%d/%m/%Y - %H:%M"))]
    pub created_at: NaiveDateTime,
    #[table(format(string = "%d/%m/%Y - %H:%M"))]
    pub updated_at: NaiveDateTime,
}

#[cfg(not(feature = "ssr"))]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub is_admin: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub last_login_at: Option<NaiveDateTime>,
    // pub roles: Vec<String>,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Clone, Default, Serialize, Deserialize, FromQueryResult)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub is_admin: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub last_login_at: Option<NaiveDateTime>,
    //pub roles: Vec<String>,
}

#[cfg(feature = "ssr")]
impl EntityInfo for entity::user::Entity {
    type View = User;

    fn filter_column() -> Self::Column {
        entity::user::Column::Username
    }

    fn index_to_column(index: usize) -> Option<Self::Column> {
        match index {
            0 => Some(entity::user::Column::Id),
            1 => Some(entity::user::Column::Username),
            2 => Some(entity::user::Column::IsAdmin),
            3 => Some(entity::user::Column::CreatedAt),
            4 => Some(entity::user::Column::UpdatedAt),
            _ => None,
        }
    }

    fn extend_query_for_access(
        query: Select<Self>,
        user: AuthenticatedUser,
        _roles: Vec<String>,
    ) -> Select<Self> {
        if user.is_admin {
            return query;
        }
        query.filter(
            Expr::col((entity::user::Entity, entity::user::Column::Id)).eq(uuid::Uuid::nil()),
        )
    }
}

impl From<User> for UserRow {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            is_admin: user.is_admin,
            username: user.username,
            created_at: user.created_at,
            updated_at: user.updated_at,
        }
    }
}
#[cfg(feature = "ssr")]
impl From<entity::user::Model> for User {
    fn from(model: entity::user::Model) -> Self {
        Self {
            id: model.id,
            is_admin: model.is_admin,
            username: model.username,
            created_at: model.created_at,
            updated_at: model.updated_at,
            last_login_at: model.last_authenticated,
            // roles: vec![],
        }
    }
}

#[cfg(feature = "ssr")]
impl From<User> for entity::user::ActiveModel {
    fn from(user: User) -> Self {
        Self {
            id: Set(user.id),
            username: Set(user.username),
            is_admin: Set(user.is_admin),
            created_at: sea_orm::NotSet,
            updated_at: sea_orm::NotSet,
            last_authenticated: sea_orm::NotSet,
        }
    }
}

impl ExtraRowTrait for UserRow {
    fn get_id(&self) -> Uuid {
        self.id
    }

    fn get_name(&self) -> String {
        self.username.clone()
    }
}

#[server]
pub async fn user_get(id: Uuid) -> Result<User, ServerFnError> {
    get_by_id::<entity::user::Entity>(id).await
}

#[server]
pub async fn user_list(query: QueryParams) -> Result<Vec<User>, ServerFnError> {
    get_all::<entity::user::Entity>(query, HashMap::new()).await
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserWithRoles {
    user: User,
    roles: Vec<String>,
}

#[server]
async fn list_users_with_roles() -> Result<Vec<UserWithRoles>, ServerFnError> {
    let db = use_context::<DatabaseConnection>()
        .ok_or(ServerFnError::new("No database connection".to_string()))?;

    let rows = entity::user::Entity::find()
        .left_join(entity::role::Entity)
        .select_also(entity::role::Entity)
        .all(&db)
        .await
        .map_err(|e| ServerFnError::new(format!("{e:?}")))?;

    let mut user_map: HashMap<Uuid, UserWithRoles> = HashMap::new();

    for (user, role) in rows {
        let entry = user_map.entry(user.id).or_insert(UserWithRoles {
            user: user.into(),
            roles: Vec::new(),
        });

        if let Some(role) = role {
            entry.roles.push(role.name);
        }
    }

    Ok(user_map.into_values().collect())
}

#[server]
pub async fn user_list_names() -> Result<HashSet<String>, ServerFnError> {
    get_all_names::<entity::user::Entity>(HashMap::new()).await
}

#[server]
pub async fn user_add(user: User) -> Result<(), ServerFnError> {
    add::<entity::user::Entity>(user).await
}

#[server]
pub async fn user_update(user: User) -> Result<(), ServerFnError> {
    update::<entity::user::Entity>(user).await
}

#[server]
pub async fn user_remove(id: Uuid) -> Result<(), ServerFnError> {
    delete_by_id::<entity::user::Entity>(id).await
}

#[server]
pub async fn user_count() -> Result<usize, ServerFnError> {
    count::<entity::user::Entity>(HashMap::new()).await
}
