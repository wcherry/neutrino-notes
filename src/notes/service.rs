use crate::common::{ApiError, AuthenticatedUser};
use crate::notes::{
    dto::{
        CreateNoteRequest, ListNotesResponse, NoteMetaResponse, NoteResponse, SaveNoteRequest,
    },
    model::{NewNoteRecord, UpdateNoteRecord},
    repository::NotesRepository,
};
use chrono::Utc;
use reqwest::Client;
use shared::drive_client::DriveClient;
use std::sync::Arc;
use uuid::Uuid;

const MIME_TYPE: &str = "application/x-neutrino-note";
const EMPTY_NOTE_CONTENT: &str = "";

pub struct NotesService {
    repo: Arc<NotesRepository>,
    drive: Arc<DriveClient>,
    drive_base_url: String,
    http: Client,
}

impl NotesService {
    pub fn new(repo: Arc<NotesRepository>, drive: Arc<DriveClient>, drive_base_url: String) -> Self {
        NotesService {
            repo,
            drive,
            drive_base_url,
            http: Client::new(),
        }
    }

    pub async fn list_notes(&self, user: &AuthenticatedUser) -> Result<ListNotesResponse, ApiError> {
        let items = self.drive.list_files(&user.token, MIME_TYPE).await?;
        let notes = items
            .into_iter()
            .map(|item| NoteMetaResponse {
                id: item.id,
                title: item.name,
                folder_id: item.folder_id,
                created_at: item.created_at.and_utc().to_rfc3339(),
                updated_at: item.updated_at.and_utc().to_rfc3339(),
            })
            .collect();
        Ok(ListNotesResponse { notes })
    }

    pub async fn create_note(
        &self,
        user: &AuthenticatedUser,
        req: CreateNoteRequest,
    ) -> Result<NoteResponse, ApiError> {
        let title = req.title.trim().to_string();
        if title.is_empty() {
            return Err(ApiError::bad_request("Note title cannot be empty"));
        }
        let id = Uuid::new_v4().to_string();
        let file = self
            .drive
            .create_file(&user.token, &id, &title, MIME_TYPE, req.folder_id.as_deref())
            .await?;

        let new_note = NewNoteRecord { file_id: &id };
        self.repo.insert_note(new_note)?;

        self.drive
            .upload_content(&user.token, &id, EMPTY_NOTE_CONTENT, "create_note_content")
            .await?;

        Ok(NoteResponse {
            id: file.id,
            title: file.name,
            content: EMPTY_NOTE_CONTENT.to_string(),
            folder_id: file.folder_id,
            created_at: file.created_at.and_utc().to_rfc3339(),
            updated_at: file.updated_at.and_utc().to_rfc3339(),
        })
    }

    pub async fn get_note(
        &self,
        user: &AuthenticatedUser,
        note_id: &str,
    ) -> Result<NoteResponse, ApiError> {
        let file = self
            .drive
            .get_file(&user.token, note_id, "Note not found")
            .await?;
        if file.deleted_at.is_some() {
            return Err(ApiError::not_found("Note is in trash"));
        }
        let content = self
            .drive
            .get_content(&user.token, note_id, "Note content not found")
            .await
            .unwrap_or_default();
        Ok(NoteResponse {
            id: file.id,
            title: file.name,
            content,
            folder_id: file.folder_id,
            created_at: file.created_at.and_utc().to_rfc3339(),
            updated_at: file.updated_at.and_utc().to_rfc3339(),
        })
    }

    pub async fn save_note(
        &self,
        user: &AuthenticatedUser,
        note_id: &str,
        req: SaveNoteRequest,
    ) -> Result<NoteMetaResponse, ApiError> {
        let file = self
            .drive
            .get_file(&user.token, note_id, "Note not found")
            .await?;
        match file.your_role.as_str() {
            "owner" | "editor" => {}
            _ => return Err(ApiError::new(403, "FORBIDDEN", "Edit access required")),
        }
        if file.deleted_at.is_some() {
            return Err(ApiError::not_found("Note is in trash"));
        }

        let new_title = if let Some(ref title) = req.title {
            let trimmed = title.trim().to_string();
            if !trimmed.is_empty() {
                self.drive
                    .update_file_name(&user.token, note_id, &trimmed)
                    .await?;
                trimmed
            } else {
                file.name.clone()
            }
        } else {
            file.name.clone()
        };

        self.drive
            .upload_content(&user.token, note_id, &req.content, "save_note_content")
            .await?;

        let now = Utc::now().naive_utc();
        let changes = UpdateNoteRecord { updated_at: now };
        self.repo.update_note(note_id, changes)?;

        Ok(NoteMetaResponse {
            id: file.id,
            title: new_title,
            folder_id: file.folder_id,
            created_at: file.created_at.and_utc().to_rfc3339(),
            updated_at: now.and_utc().to_rfc3339(),
        })
    }

    pub async fn delete_note(
        &self,
        user: &AuthenticatedUser,
        note_id: &str,
    ) -> Result<(), ApiError> {
        // Verify access before trashing
        let file = self
            .drive
            .get_file(&user.token, note_id, "Note not found")
            .await?;
        match file.your_role.as_str() {
            "owner" => {}
            _ => return Err(ApiError::new(403, "FORBIDDEN", "Only the owner can delete a note")),
        }

        // Trash in drive
        let url = format!("{}/api/v1/drive/files/{}", self.drive_base_url, note_id);
        let resp = self
            .http
            .delete(&url)
            .bearer_auth(&user.token)
            .send()
            .await
            .map_err(|e| {
                tracing::error!("Drive trash note error: {:?}", e);
                ApiError::internal("Failed to reach drive service")
            })?;
        if !resp.status().is_success() {
            tracing::error!("Drive trash note returned {}", resp.status());
            return Err(ApiError::internal("Drive service error"));
        }

        self.repo.delete_note(note_id)?;
        Ok(())
    }
}
