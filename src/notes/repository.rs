use crate::common::ApiError;
use crate::notes::model::{NewNoteRecord, NoteRecord, UpdateNoteRecord};
use crate::schema::notes;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};

pub type DbPool = Pool<ConnectionManager<SqliteConnection>>;

pub struct NotesRepository {
    pool: DbPool,
}

impl NotesRepository {
    pub fn new(pool: DbPool) -> Self {
        NotesRepository { pool }
    }

    fn get_conn(
        &self,
    ) -> Result<diesel::r2d2::PooledConnection<ConnectionManager<SqliteConnection>>, ApiError> {
        self.pool.get().map_err(|e| {
            tracing::error!("DB pool error: {:?}", e);
            ApiError::internal("Database connection unavailable")
        })
    }

    pub fn insert_note(&self, new_note: NewNoteRecord) -> Result<NoteRecord, ApiError> {
        let mut conn = self.get_conn()?;
        diesel::insert_into(notes::table)
            .values(&new_note)
            .execute(&mut conn)
            .map_err(|e| {
                tracing::error!("DB insert note error: {:?}", e);
                ApiError::internal("Database error")
            })?;
        notes::table
            .filter(notes::file_id.eq(new_note.file_id))
            .select(NoteRecord::as_select())
            .first(&mut conn)
            .map_err(|e| {
                tracing::error!("DB query after note insert error: {:?}", e);
                ApiError::internal("Database error")
            })
    }

    pub fn update_note(
        &self,
        file_id: &str,
        changes: UpdateNoteRecord,
    ) -> Result<NoteRecord, ApiError> {
        let mut conn = self.get_conn()?;
        diesel::update(notes::table.filter(notes::file_id.eq(file_id)))
            .set(&changes)
            .execute(&mut conn)
            .map_err(|e| {
                tracing::error!("DB update note error: {:?}", e);
                ApiError::internal("Database error")
            })?;
        notes::table
            .filter(notes::file_id.eq(file_id))
            .select(NoteRecord::as_select())
            .first(&mut conn)
            .map_err(|e| {
                tracing::error!("DB get note after update error: {:?}", e);
                ApiError::internal("Database error")
            })
    }

    pub fn delete_note(&self, file_id: &str) -> Result<(), ApiError> {
        let mut conn = self.get_conn()?;
        diesel::delete(notes::table.filter(notes::file_id.eq(file_id)))
            .execute(&mut conn)
            .map_err(|e| {
                tracing::error!("DB delete note error: {:?}", e);
                ApiError::internal("Database error")
            })?;
        Ok(())
    }
}
