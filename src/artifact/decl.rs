use crate::artifact::ArtifactError;
use failure::Error;

/// The kind of declaration this is
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Decl {
    /// Declaration of an import
    Import(ImportKind),
    /// Declaration of an item to be defined in this artifact
    Defined(DefinedDecl),
}

/// The kind of import this is - either a function, or a copy relocation of data from a shared library
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ImportKind {
    /// A function
    Function,
    /// An imported piece of data
    Data,
}

impl ImportKind {
    /// Accessor for the ImportKind associated with a Decl, if there is one
    pub fn from_decl(decl: &Decl) -> Option<Self> {
        match decl {
            Decl::Import(ik) => Some(*ik),
            _ => None,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
/// A declaration that is defined inside this artifact
pub enum DefinedDecl {
    /// A function defined in this artifact
    Function(FunctionDecl),
    /// A data object defined in this artifact
    Data(DataDecl),
    /// A null-terminated string object defined in this artifact
    CString(CStringDecl),
    /// A DWARF debug section defined in this artifact
    DebugSection(DebugSectionDecl),
}

impl DefinedDecl {
    /// Accessor to determine whether variant is Function
    pub fn is_function(&self) -> bool {
        match self {
            DefinedDecl::Function { .. } => true,
            _ => false,
        }
    }

    /// Accessor to determine whether variant is Data
    pub fn is_data(&self) -> bool {
        match self {
            DefinedDecl::Data { .. } => true,
            _ => false,
        }
    }

    /// Accessor to determine whether variant is CString
    pub fn is_cstring(&self) -> bool {
        match self {
            DefinedDecl::CString { .. } => true,
            _ => false,
        }
    }

    /// Accessor to determine whether variant is DebugSection
    pub fn is_debug_section(&self) -> bool {
        match self {
            DefinedDecl::DebugSection { .. } => true,
            _ => false,
        }
    }

    /// Accessor to determine whether scope is global
    pub fn is_global(&self) -> bool {
        match self {
            DefinedDecl::Function(a) => a.is_global(),
            DefinedDecl::Data(a) => a.is_global(),
            DefinedDecl::CString(a) => a.is_global(),
            DefinedDecl::DebugSection(a) => a.is_global(),
        }
    }

    /// Accessor to determine whether contents are writable
    pub fn is_writable(&self) -> bool {
        match self {
            DefinedDecl::Data(a) => a.is_writable(),
            DefinedDecl::Function(_) | DefinedDecl::CString(_) | DefinedDecl::DebugSection(_) => {
                false
            }
        }
    }
}

impl Decl {
    /// An import of a function/routine defined in a shared library
    pub fn function_import() -> FunctionImportDecl {
        FunctionImportDecl::default()
    }
    /// A GOT-based import of data defined in a shared library
    pub fn data_import() -> DataImportDecl {
        DataImportDecl::default()
    }
    /// A function defined in this artifact
    pub fn function() -> FunctionDecl {
        FunctionDecl::default()
    }
    /// A data object defined in this artifact
    pub fn data() -> DataDecl {
        DataDecl::default()
    }
    /// A null-terminated string object defined in this artifact
    pub fn cstring() -> CStringDecl {
        CStringDecl::default()
    }
    /// A DWARF debug section defined in this artifact
    pub fn debug_section() -> DebugSectionDecl {
        DebugSectionDecl::default()
    }

    /// If it is compatible, absorb the new declaration (`other`) into the old (`self`); otherwise returns an error.
    ///
    /// The rule here is "C-ish", but essentially:
    ///
    /// 1. Duplicate declarations are no-ops / ignored.
    /// 2. **If** the previous declaration was an [FunctionImport](enum.Decl.html#variant.FunctionImport) or [DataImport](enum.Decl.html#variant.DataImport),
    ///    **then** if the subsequent declaration is a corresponding matching [Function](enum.Decl.html#variant.Function) or [Data](enum.Decl.html#variant.Data)
    ///    declaration, it is said to be "upgraded", and forever after is considered a declaration in need of a definition.
    /// 3. **If** the previous declaration was a `Function` or `Data` declaration,
    ///    **then** a subsequent corresponding `FunctionImport` or `DataImport` is a no-op.
    /// 4. Anything else is a [IncompatibleDeclaration](enum.ArtifactError.html#variant.IncompatibleDeclaration) error!
    // ref https://github.com/m4b/faerie/issues/24
    // ref https://github.com/m4b/faerie/issues/18
    pub fn absorb(&mut self, other: Self) -> Result<(), Error> {
        // FIXME: i can't think of a way offhand to not clone here, without unusual contortions
        match self.clone() {
            Decl::Import(ImportKind::Data) => {
                match other {
                    // data imports can be upgraded to any kind of data declaration
                    Decl::Defined(DefinedDecl::Data { .. }) => {
                        *self = other;
                        Ok(())
                    }
                    Decl::Import(ImportKind::Data) => Ok(()),
                    _ => Err(ArtifactError::IncompatibleDeclaration {
                        old: *self,
                        new: other,
                    }
                    .into()),
                }
            }
            Decl::Import(ImportKind::Function) => {
                match other {
                    // function imports can be upgraded to any kind of function declaration
                    Decl::Defined(DefinedDecl::Function { .. }) => {
                        *self = other;
                        Ok(())
                    }
                    Decl::Import(ImportKind::Function) => Ok(()),
                    _ => Err(ArtifactError::IncompatibleDeclaration {
                        old: *self,
                        new: other,
                    }
                    .into()),
                }
            }
            // a previous data declaration can only be re-declared a data import, or it must match exactly the
            // next declaration
            decl @ Decl::Defined(DefinedDecl::Data { .. }) => match other {
                Decl::Import(ImportKind::Data) => Ok(()),
                other => {
                    if decl == other {
                        Ok(())
                    } else {
                        Err(ArtifactError::IncompatibleDeclaration {
                            old: *self,
                            new: other,
                        }
                        .into())
                    }
                }
            },
            // a previous function decl can only be re-declared a function import, or it must match exactly
            // the next declaration
            decl @ Decl::Defined(DefinedDecl::Function { .. }) => match other {
                Decl::Import(ImportKind::Function) => Ok(()),
                other => {
                    if decl == other {
                        Ok(())
                    } else {
                        Err(ArtifactError::IncompatibleDeclaration {
                            old: *self,
                            new: other,
                        }
                        .into())
                    }
                }
            },
            decl => {
                if decl == other {
                    Ok(())
                } else {
                    Err(ArtifactError::IncompatibleDeclaration {
                        old: *self,
                        new: other,
                    }
                    .into())
                }
            }
        }
    }
    /// Is this an import (function or data) from a shared library?
    pub fn is_import(&self) -> bool {
        match *self {
            Decl::Import(_) => true,
            _ => false,
        }
    }
    /// Is this a section?
    pub fn is_section(&self) -> bool {
        match *self {
            Decl::Defined(DefinedDecl::DebugSection { .. }) => true,
            _ => false,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
/// Builder for function import declarations
pub struct FunctionImportDecl {}

impl Default for FunctionImportDecl {
    fn default() -> Self {
        FunctionImportDecl {}
    }
}

impl Into<Decl> for FunctionImportDecl {
    fn into(self) -> Decl {
        Decl::Import(ImportKind::Function)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
/// Builder for data import declarations
pub struct DataImportDecl {}

impl Default for DataImportDecl {
    fn default() -> Self {
        DataImportDecl {}
    }
}

impl Into<Decl> for DataImportDecl {
    fn into(self) -> Decl {
        Decl::Import(ImportKind::Data)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
/// Builder for function declarations
pub struct FunctionDecl {
    global: bool,
}

impl Default for FunctionDecl {
    fn default() -> Self {
        FunctionDecl { global: false }
    }
}

impl FunctionDecl {
    /// Set binding to global
    pub fn global(mut self) -> Self {
        self.global = true;
        self
    }
    /// Set binding to local
    pub fn local(mut self) -> Self {
        self.global = false;
        self
    }
    /// Accessor for binding
    pub fn is_global(&self) -> bool {
        self.global
    }
}

impl Into<Decl> for FunctionDecl {
    fn into(self) -> Decl {
        Decl::Defined(DefinedDecl::Function(self))
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
/// Builder for data declarations
pub struct DataDecl {
    global: bool,
    writable: bool,
}

impl Default for DataDecl {
    fn default() -> Self {
        DataDecl {
            global: false,
            writable: false,
        }
    }
}

impl DataDecl {
    /// Set binding to global
    pub fn global(mut self) -> Self {
        self.global = true;
        self
    }
    /// Set binding to local
    pub fn local(mut self) -> Self {
        self.global = false;
        self
    }
    /// Accessor for binding
    pub fn is_global(&self) -> bool {
        self.global
    }
    /// Set mutability to writable
    pub fn writable(mut self) -> Self {
        self.writable = true;
        self
    }
    /// Set mutability to read-only
    pub fn read_only(mut self) -> Self {
        self.writable = false;
        self
    }
    /// Accessor for mutability
    pub fn is_writable(&self) -> bool {
        self.writable
    }
}

impl Into<Decl> for DataDecl {
    fn into(self) -> Decl {
        Decl::Defined(DefinedDecl::Data(self))
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
/// Builder for a CString (0-terminated character sequence) declaration
pub struct CStringDecl {
    global: bool,
}

impl Default for CStringDecl {
    fn default() -> Self {
        CStringDecl { global: false }
    }
}

impl CStringDecl {
    /// Set binding to global
    pub fn global(mut self) -> Self {
        self.global = true;
        self
    }
    /// Set binding to local
    pub fn local(mut self) -> Self {
        self.global = false;
        self
    }
    /// Accessor for binding
    pub fn is_global(&self) -> bool {
        self.global
    }
}

impl Into<Decl> for CStringDecl {
    fn into(self) -> Decl {
        Decl::Defined(DefinedDecl::CString(self))
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
/// Builder for a debug section declaration
pub struct DebugSectionDecl {}

impl DebugSectionDecl {
    /// Debug sections are never global, but we have an accessor
    /// for symmetry with other section declarations
    pub fn is_global(&self) -> bool {
        false
    }
}

impl Default for DebugSectionDecl {
    fn default() -> Self {
        DebugSectionDecl {}
    }
}

impl Into<Decl> for DebugSectionDecl {
    fn into(self) -> Decl {
        Decl::Defined(DefinedDecl::DebugSection(self))
    }
}