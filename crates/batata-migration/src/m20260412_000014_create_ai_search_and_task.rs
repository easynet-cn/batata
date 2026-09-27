//! AI resource search index and async task tables.
//!
//! Mirrors the upstream Nacos 3.2.0 AI schema:
//! - `ai_resource_search_document` — one search document per resource version
//! - `ai_resource_search_chunk`    — chunks split out of a search document
//! - `ai_resource_task`            — lease-based async task queue
//!
//! Upstream DDL reference:
//! - MySQL: `nacos/plugin-default-impl/.../nacos-datasource-plugin-mysql/
//!   src/main/resources/META-INF/mysql-schema.sql:249-321`
//! - PostgreSQL: `nacos/plugin-default-impl/.../nacos-datasource-plugin-postgresql/
//!   src/main/resources/META-INF/pg-schema.sql:570-666`
//!
//! The optional pgvector table `ai_resource_search_embedding_pg` is
//! deliberately **not** created here.

use crate::column_helper::{long_text, long_text_null};
use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_database_backend();

        // =====================================================================
        // ai_resource_search_document
        // =====================================================================
        manager
            .create_table(
                Table::create()
                    .table(AiResourceSearchDocument::Table)
                    .if_not_exists()
                    .col(
                        big_integer(AiResourceSearchDocument::Id)
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        date_time(AiResourceSearchDocument::GmtCreate)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        date_time(AiResourceSearchDocument::GmtModified)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        string_len(AiResourceSearchDocument::NamespaceId, 128)
                            .not_null()
                            .default(""),
                    )
                    .col(
                        string_len(AiResourceSearchDocument::ResourceType, 32).not_null(),
                    )
                    .col(
                        string_len(AiResourceSearchDocument::ResourceName, 256)
                            .not_null(),
                    )
                    .col(
                        string_len(AiResourceSearchDocument::ResourceVersion, 64)
                            .not_null(),
                    )
                    .col(
                        string_len(AiResourceSearchDocument::DisplayName, 256)
                            .not_null(),
                    )
                    .col(string_len_null(AiResourceSearchDocument::CDesc, 2048))
                    .col(long_text_null(AiResourceSearchDocument::Tags, backend))
                    .col(long_text_null(
                        AiResourceSearchDocument::Capabilities,
                        backend,
                    ))
                    .col(long_text_null(
                        AiResourceSearchDocument::RepresentativeQueries,
                        backend,
                    ))
                    .col(long_text_null(AiResourceSearchDocument::Metadata, backend))
                    .col(
                        string_len(AiResourceSearchDocument::SourceDigest, 64)
                            .not_null(),
                    )
                    .col(string_len(AiResourceSearchDocument::Status, 32).not_null())
                    .col(
                        string_len(AiResourceSearchDocument::GenerateMode, 32)
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // UNIQUE KEY `uk_search_document_resource_version`
        manager
            .create_index(
                Index::create()
                    .name("uk_search_document_resource_version")
                    .table(AiResourceSearchDocument::Table)
                    .col(AiResourceSearchDocument::NamespaceId)
                    .col(AiResourceSearchDocument::ResourceType)
                    .col(AiResourceSearchDocument::ResourceName)
                    .col(AiResourceSearchDocument::ResourceVersion)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // KEY `idx_search_document_type_status`
        manager
            .create_index(
                Index::create()
                    .name("idx_search_document_type_status")
                    .table(AiResourceSearchDocument::Table)
                    .col(AiResourceSearchDocument::NamespaceId)
                    .col(AiResourceSearchDocument::ResourceType)
                    .col(AiResourceSearchDocument::Status)
                    .col(AiResourceSearchDocument::ResourceName)
                    .col(AiResourceSearchDocument::Id)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // =====================================================================
        // ai_resource_search_chunk
        // =====================================================================
        manager
            .create_table(
                Table::create()
                    .table(AiResourceSearchChunk::Table)
                    .if_not_exists()
                    .col(
                        big_integer(AiResourceSearchChunk::Id)
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        date_time(AiResourceSearchChunk::GmtCreate)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        date_time(AiResourceSearchChunk::GmtModified)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(big_integer(AiResourceSearchChunk::DocumentId).not_null())
                    .col(
                        string_len(AiResourceSearchChunk::NamespaceId, 128)
                            .not_null()
                            .default(""),
                    )
                    .col(string_len(AiResourceSearchChunk::ResourceType, 32).not_null())
                    .col(string_len(AiResourceSearchChunk::ResourceName, 256).not_null())
                    .col(string_len(AiResourceSearchChunk::ResourceVersion, 64).not_null())
                    .col(string_len(AiResourceSearchChunk::ChunkType, 64).not_null())
                    .col(long_text(AiResourceSearchChunk::ChunkText, backend))
                    .col(long_text(AiResourceSearchChunk::CanonicalText, backend))
                    .col(string_len_null(AiResourceSearchChunk::Language, 16))
                    .col(string_len(AiResourceSearchChunk::ChunkHash, 64).not_null())
                    .col(long_text_null(AiResourceSearchChunk::Metadata, backend))
                    .col(string_len(AiResourceSearchChunk::Status, 32).not_null())
                    .to_owned(),
            )
            .await?;

        // KEY `idx_search_chunk_document` (`document_id`)
        manager
            .create_index(
                Index::create()
                    .name("idx_search_chunk_document")
                    .table(AiResourceSearchChunk::Table)
                    .col(AiResourceSearchChunk::DocumentId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // KEY `idx_search_chunk_hash` (`chunk_hash`)
        manager
            .create_index(
                Index::create()
                    .name("idx_search_chunk_hash")
                    .table(AiResourceSearchChunk::Table)
                    .col(AiResourceSearchChunk::ChunkHash)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // KEY `idx_search_chunk_resource`
        manager
            .create_index(
                Index::create()
                    .name("idx_search_chunk_resource")
                    .table(AiResourceSearchChunk::Table)
                    .col(AiResourceSearchChunk::NamespaceId)
                    .col(AiResourceSearchChunk::ResourceType)
                    .col(AiResourceSearchChunk::ResourceName)
                    .col(AiResourceSearchChunk::ResourceVersion)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // KEY `idx_search_chunk_type_status`
        manager
            .create_index(
                Index::create()
                    .name("idx_search_chunk_type_status")
                    .table(AiResourceSearchChunk::Table)
                    .col(AiResourceSearchChunk::NamespaceId)
                    .col(AiResourceSearchChunk::ResourceType)
                    .col(AiResourceSearchChunk::Status)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // =====================================================================
        // ai_resource_task
        // =====================================================================
        manager
            .create_table(
                Table::create()
                    .table(AiResourceTask::Table)
                    .if_not_exists()
                    .col(
                        string_len(AiResourceTask::TaskKey, 64)
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        string_len(AiResourceTask::NamespaceId, 128)
                            .not_null()
                            .default(""),
                    )
                    .col(string_len(AiResourceTask::TaskType, 64).not_null())
                    .col(string_len(AiResourceTask::TaskStage, 32).not_null())
                    .col(string_len(AiResourceTask::Status, 16).not_null())
                    .col(long_text(AiResourceTask::TaskPayload, backend))
                    .col(long_text_null(AiResourceTask::TaskResult, backend))
                    .col(integer(AiResourceTask::RetryCount).not_null().default(0))
                    .col(big_integer(AiResourceTask::Revision).not_null().default(1))
                    .col(big_integer(AiResourceTask::LeaseToken).not_null().default(0))
                    .col(big_integer(AiResourceTask::NextExecuteAt).not_null())
                    .col(big_integer_null(AiResourceTask::LeaseExpireAt))
                    .col(string_len_null(AiResourceTask::LastError, 2000))
                    .col(
                        date_time(AiResourceTask::GmtCreate)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        date_time(AiResourceTask::GmtModified)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // KEY `idx_ai_resource_task_due` (`task_type`,`status`,`next_execute_at`)
        manager
            .create_index(
                Index::create()
                    .name("idx_ai_resource_task_due")
                    .table(AiResourceTask::Table)
                    .col(AiResourceTask::TaskType)
                    .col(AiResourceTask::Status)
                    .col(AiResourceTask::NextExecuteAt)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // KEY `idx_ai_resource_task_lease` (`task_type`,`status`,`lease_expire_at`)
        manager
            .create_index(
                Index::create()
                    .name("idx_ai_resource_task_lease")
                    .table(AiResourceTask::Table)
                    .col(AiResourceTask::TaskType)
                    .col(AiResourceTask::Status)
                    .col(AiResourceTask::LeaseExpireAt)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AiResourceTask::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(AiResourceSearchChunk::Table).to_owned())
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(AiResourceSearchDocument::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum AiResourceSearchDocument {
    Table,
    Id,
    GmtCreate,
    GmtModified,
    NamespaceId,
    ResourceType,
    ResourceName,
    ResourceVersion,
    DisplayName,
    CDesc,
    Tags,
    Capabilities,
    RepresentativeQueries,
    Metadata,
    SourceDigest,
    Status,
    GenerateMode,
}

#[derive(DeriveIden)]
enum AiResourceSearchChunk {
    Table,
    Id,
    GmtCreate,
    GmtModified,
    DocumentId,
    NamespaceId,
    ResourceType,
    ResourceName,
    ResourceVersion,
    ChunkType,
    ChunkText,
    CanonicalText,
    Language,
    ChunkHash,
    Metadata,
    Status,
}

#[derive(DeriveIden)]
enum AiResourceTask {
    Table,
    TaskKey,
    NamespaceId,
    TaskType,
    TaskStage,
    Status,
    TaskPayload,
    TaskResult,
    RetryCount,
    Revision,
    LeaseToken,
    NextExecuteAt,
    LeaseExpireAt,
    LastError,
    GmtCreate,
    GmtModified,
}
