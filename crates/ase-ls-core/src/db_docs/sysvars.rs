//! ASE システム変数ドキュメントデータ

use super::{DocCategory, DocEntry};

/// ASE システム変数エントリ
pub static SYSTEM_VARIABLE_ENTRIES: &[DocEntry] = &[
    DocEntry {
        name: "@@IDENTITY",
        description: "Last identity value inserted",
        syntax: "@@IDENTITY",
        params: &[],
        category: DocCategory::SystemVariable,
    },
    DocEntry {
        name: "@@ROWCOUNT",
        description: "Number of rows affected by the last statement",
        syntax: "@@ROWCOUNT",
        params: &[],
        category: DocCategory::SystemVariable,
    },
    DocEntry {
        name: "@@ERROR",
        description: "Error number for the last T-SQL statement executed",
        syntax: "@@ERROR",
        params: &[],
        category: DocCategory::SystemVariable,
    },
    DocEntry {
        name: "@@VERSION",
        description: "Version information of the ASE server",
        syntax: "@@VERSION",
        params: &[],
        category: DocCategory::SystemVariable,
    },
    DocEntry {
        name: "@@SERVERNAME",
        description: "Name of the local server",
        syntax: "@@SERVERNAME",
        params: &[],
        category: DocCategory::SystemVariable,
    },
    DocEntry {
        name: "@@SPID",
        description: "Server process ID for the current session",
        syntax: "@@SPID",
        params: &[],
        category: DocCategory::SystemVariable,
    },
    DocEntry {
        name: "@@TRANCOUNT",
        description: "Number of active transactions for the current session",
        syntax: "@@TRANCOUNT",
        params: &[],
        category: DocCategory::SystemVariable,
    },
    DocEntry {
        name: "@@DATEFIRST",
        description: "First day of the week (1=Monday, 7=Sunday)",
        syntax: "@@DATEFIRST",
        params: &[],
        category: DocCategory::SystemVariable,
    },
];
