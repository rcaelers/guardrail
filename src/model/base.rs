use sea_orm::*;

pub trait HasId {
    fn id(&self) -> uuid::Uuid;
}
pub struct Repo;

impl Repo {
    pub async fn create<E, D, A>(db: &DbConn, data: D) -> Result<uuid::Uuid, DbErr>
    where
        E: EntityTrait,
        E::Model: IntoActiveModel<A> + HasId,
        D: IntoActiveModel<A>,
        A: ActiveModelTrait<Entity = E> + ActiveModelBehavior + Send,
    {
        let model = data.into_active_model().insert(db).await?;
        Ok(model.id())
    }

    pub async fn update<E, D, A>(db: &DbConn, data: D) -> Result<uuid::Uuid, DbErr>
    where
        E: EntityTrait,
        E::Model: IntoActiveModel<A> + HasId,
        D: IntoActiveModel<A>,
        A: ActiveModelTrait<Entity = E> + ActiveModelBehavior + Send,
    {
        // let now = chrono::NaiveDateTime::from_timestamp_opt(chrono::Utc::now().timestamp(), 0)
        //     .ok_or(DbErr::Custom("invalid timestamp".to_owned()))?;
        let model = data.into_active_model().update(db).await?;
        Ok(model.id())
    }

    pub async fn delete_by_id<E>(db: &DbConn, id: uuid::Uuid) -> Result<(), DbErr>
    where
        E: EntityTrait,
        <<E as sea_orm::EntityTrait>::PrimaryKey as sea_orm::PrimaryKeyTrait>::ValueType:
            From<uuid::Uuid>,
    {
        <E as EntityTrait>::delete_by_id(id).exec(db).await?;
        Ok(())
    }

    pub async fn get_all<E>(db: &DbConn) -> Result<Vec<<E as EntityTrait>::Model>, DbErr>
    where
        E: EntityTrait,
    {
        <E as EntityTrait>::find().all(db).await
    }

    pub async fn get_by_id<E>(
        db: &DbConn,
        id: uuid::Uuid,
    ) -> Result<Option<<E as EntityTrait>::Model>, DbErr>
    where
        E: EntityTrait,
        <E::PrimaryKey as sea_orm::PrimaryKeyTrait>::ValueType: From<uuid::Uuid>,
    {
        <E as EntityTrait>::find_by_id(id).one(db).await
    }

    pub async fn get_by_column<E, Id, C>(
        db: &DbConn,
        column: C,
        key: Id,
    ) -> Result<Option<<E as EntityTrait>::Model>, DbErr>
    where
        E: EntityTrait,
        Id: Into<sea_orm::Value>,
        C: ColumnTrait + Clone + Sync + Send,
    {
        E::find().filter(column.eq(key)).one(db).await
    }

    pub async fn get_all_by_column<E, Id, C>(
        db: &DbConn,
        column: C,
        key: Id,
    ) -> Result<Vec<<E as EntityTrait>::Model>, DbErr>
    where
        E: EntityTrait,
        Id: Into<sea_orm::Value>,
        C: ColumnTrait + Clone + Sync + Send,
    {
        E::find().filter(column.eq(key)).all(db).await
    }
}
