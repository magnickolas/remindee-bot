use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Reminder::Table)
                    .add_column(
                        ColumnDef::new(Reminder::NagIntervalSec).big_integer(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(ReminderMessage::Table)
                    .add_column(
                        ColumnDef::new(ReminderMessage::IsDelivery)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ReminderOccurrence::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ReminderOccurrence::Id)
                            .integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(ReminderOccurrence::RecId)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ReminderOccurrence::ChatId)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(ReminderOccurrence::UserId).integer())
                    .col(
                        ColumnDef::new(ReminderOccurrence::DueAt)
                            .date_time()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ReminderOccurrence::NextNagAt)
                            .date_time()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ReminderOccurrence::NagIntervalSec)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(ReminderOccurrence::StopAt).date_time())
                    .col(ColumnDef::new(ReminderOccurrence::DoneAt).date_time())
                    .col(
                        ColumnDef::new(ReminderOccurrence::ClosedReason)
                            .string(),
                    )
                    .col(
                        ColumnDef::new(ReminderOccurrence::DescSnapshot)
                            .text()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("ix_rem_occ_done_next")
                    .table(ReminderOccurrence::Table)
                    .col(ReminderOccurrence::DoneAt)
                    .col(ReminderOccurrence::NextNagAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("ix_rem_occ_done_stop")
                    .table(ReminderOccurrence::Table)
                    .col(ReminderOccurrence::DoneAt)
                    .col(ReminderOccurrence::StopAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("ix_rem_occ_rec_done")
                    .table(ReminderOccurrence::Table)
                    .col(ReminderOccurrence::RecId)
                    .col(ReminderOccurrence::DoneAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(Index::drop().name("ix_rem_occ_done_next").to_owned())
            .await?;

        manager
            .drop_index(Index::drop().name("ix_rem_occ_rec_done").to_owned())
            .await?;

        manager
            .drop_index(Index::drop().name("ix_rem_occ_done_stop").to_owned())
            .await?;

        manager
            .drop_table(
                Table::drop().table(ReminderOccurrence::Table).to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(ReminderMessage::Table)
                    .drop_column(ReminderMessage::IsDelivery)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Reminder::Table)
                    .drop_column(Reminder::NagIntervalSec)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum Reminder {
    Table,
    NagIntervalSec,
}

#[derive(Iden)]
enum ReminderMessage {
    Table,
    IsDelivery,
}

#[derive(Iden)]
enum ReminderOccurrence {
    Table,
    Id,
    RecId,
    ChatId,
    UserId,
    DueAt,
    NextNagAt,
    NagIntervalSec,
    StopAt,
    DoneAt,
    ClosedReason,
    DescSnapshot,
}
