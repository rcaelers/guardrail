use crate::classes::ClassesPreset;
use crate::data::QueryParams;
#[cfg(feature = "ssr")]
use crate::data::{
    add, count, delete_by_id, get_all, get_all_names, get_by_id, update, EntityInfo,
};
#[cfg(feature = "ssr")]
use crate::entity;
use ::chrono::NaiveDateTime;
use leptos::*;
use leptos_struct_table::*;
use serde::{Deserialize, Serialize};
#[cfg(feature = "ssr")]
use std::collections::HashMap;
use std::collections::{HashSet, VecDeque};
use std::ops::Range;
use uuid::Uuid;

#[cfg(feature = "ssr")]
use sea_orm::*;

use super::{ExtraRowTrait, ExtraTableDataProvider};

#[derive(TableRow, Debug, Clone)]
#[table(sortable, classes_provider = ClassesPreset)]
pub struct UserRow {
    pub id: Uuid,
    pub username: String,
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
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub last_login_at: Option<NaiveDateTime>,
    pub roles: Vec<String>,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Clone, Default, Serialize, Deserialize, FromQueryResult)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub last_login_at: Option<NaiveDateTime>,
    pub roles: Vec<String>,
}
#[cfg(feature = "ssr")]
impl EntityInfo for entity::user::Entity {
    type View = User;

    fn filter_column() -> Self::Column {
        entity::user::Column::Username
    }

    fn from_index(index: usize) -> Option<Self::Column> {
        match index {
            0 => Some(entity::user::Column::Id),
            1 => Some(entity::user::Column::Username),
            2 => Some(entity::user::Column::CreatedAt),
            3 => Some(entity::user::Column::UpdatedAt),
            _ => None,
        }
    }
}

#[cfg(feature = "ssr")]
impl From<entity::user::Model> for User {
    fn from(model: entity::user::Model) -> Self {
        Self {
            id: model.id,
            username: model.username,
            created_at: model.created_at,
            updated_at: model.updated_at,
            last_login_at: model.last_authenticated,
            roles: vec![],
        }
    }
}

#[cfg(feature = "ssr")]
impl From<User> for entity::user::ActiveModel {
    fn from(user: User) -> Self {
        Self {
            id: Set(user.id),
            username: Set(user.username),
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

#[derive(Debug, Clone)]
pub struct UserTableDataProvider {
    sort: VecDeque<(usize, ColumnSort)>,
    name: RwSignal<String>,
    update: RwSignal<u64>,
}

impl UserTableDataProvider {
    pub fn new() -> Self {
        Self {
            sort: VecDeque::new(),
            name: RwSignal::new("".to_string()),
            update: RwSignal::new(0),
        }
    }
}

impl ExtraTableDataProvider<UserRow> for UserTableDataProvider {
    fn get_filter_signal(&self) -> RwSignal<String> {
        self.name
    }

    fn update(&self) {
        self.update.set(self.update.get() + 1);
    }
}

impl Default for UserTableDataProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl TableDataProvider<UserRow> for UserTableDataProvider {
    async fn get_rows(&self, range: Range<usize>) -> Result<(Vec<UserRow>, Range<usize>), String> {
        let users = user_list(QueryParams {
            filter: self.name.get_untracked().trim().to_string(),
            sorting: self.sort.clone(),
            range: range.clone(),
        })
        .await
        .map_err(|e| format!("{e:?}"))?
        .into_iter()
        .map(|user| UserRow {
            id: user.id,
            username: user.username.clone(),
            created_at: user.created_at,
            updated_at: user.updated_at,
        })
        .collect::<Vec<UserRow>>();

        let len = users.len();
        Ok((users, range.start..range.start + len))
    }

    async fn row_count(&self) -> Option<usize> {
        user_count().await.ok()
    }

    fn set_sorting(&mut self, sorting: &VecDeque<(usize, ColumnSort)>) {
        self.sort = sorting.clone();
    }

    fn track(&self) {
        self.name.track();
        self.update.track();
    }
}

#[server]
pub async fn user_get(id: Uuid) -> Result<User, ServerFnError<String>> {
    get_by_id::<User, entity::user::Entity>(id).await
}

#[server]
pub async fn user_list(query: QueryParams) -> Result<Vec<User>, ServerFnError<String>> {
    get_all::<User, entity::user::Entity>(query, vec![]).await
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserWithRoles {
    user: User,
    roles: Vec<String>,
}

#[server]
async fn list_users_with_roles() -> Result<Vec<UserWithRoles>, ServerFnError<String>> {
    let db = use_context::<DatabaseConnection>().ok_or(ServerFnError::WrappedServerError(
        "No database connection".to_string(),
    ))?;

    let rows = entity::user::Entity::find()
        .left_join(entity::role::Entity)
        .select_also(entity::role::Entity)
        .all(&db)
        .await
        .map_err(|e| ServerFnError::WrappedServerError(format!("{e:?}")))?;

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
pub async fn user_list_names() -> Result<HashSet<String>, ServerFnError<String>> {
    get_all_names::<entity::user::Entity>(vec![]).await
}

#[server]
pub async fn user_add(user: User) -> Result<(), ServerFnError<String>> {
    add::<User, entity::user::Entity>(user).await
}

#[server]
pub async fn user_update(user: User) -> Result<(), ServerFnError<String>> {
    update::<User, entity::user::Entity>(user).await
}

#[server]
pub async fn user_remove(id: Uuid) -> Result<(), ServerFnError<String>> {
    delete_by_id::<entity::user::Entity>(id).await
}

#[server]
pub async fn user_count() -> Result<usize, ServerFnError<String>> {
    count::<entity::user::Entity>(vec![]).await
}
