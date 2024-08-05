use alloc::boxed::Box;
use alloc::string::String;
use serde::{Deserialize, Serialize};
use uefi::table::runtime::VariableAttributes;

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct VarAttributes {
    pub non_volatile: bool,
    pub bootservice_access: bool,
    pub runtime_access: bool,
    pub hardware_error_record: bool,
    pub authenticated_write_access: bool,
    pub time_based_authenticated_write_access: bool,
    pub enhanced_authenticated_access: bool
}

impl Into<VariableAttributes> for VarAttributes {
    fn into(self) -> VariableAttributes {
        let mut a: VariableAttributes = VariableAttributes::empty();
        
        a.set(VariableAttributes::NON_VOLATILE, self.non_volatile);
        a.set(VariableAttributes::BOOTSERVICE_ACCESS, self.bootservice_access);
        a.set(VariableAttributes::RUNTIME_ACCESS, self.runtime_access);
        a.set(VariableAttributes::HARDWARE_ERROR_RECORD, self.hardware_error_record);
        a.set(VariableAttributes::AUTHENTICATED_WRITE_ACCESS, self.authenticated_write_access);
        a.set(VariableAttributes::TIME_BASED_AUTHENTICATED_WRITE_ACCESS, self.time_based_authenticated_write_access);
        a.set(VariableAttributes::ENHANCED_AUTHENTICATED_ACCESS, self.enhanced_authenticated_access);
        
        a
    }
}

impl From<VariableAttributes> for VarAttributes {
fn from(attr: VariableAttributes) -> VarAttributes {
    VarAttributes {
        non_volatile: attr.contains(VariableAttributes::NON_VOLATILE),
        bootservice_access: attr.contains(VariableAttributes::BOOTSERVICE_ACCESS),
        runtime_access: attr.contains(VariableAttributes::RUNTIME_ACCESS),
        hardware_error_record: attr.contains(VariableAttributes::HARDWARE_ERROR_RECORD),
        authenticated_write_access: attr.contains(VariableAttributes::AUTHENTICATED_WRITE_ACCESS),
        time_based_authenticated_write_access: attr.contains(VariableAttributes::TIME_BASED_AUTHENTICATED_WRITE_ACCESS),
        enhanced_authenticated_access: attr.contains(VariableAttributes::ENHANCED_AUTHENTICATED_ACCESS),
    }
}
}


#[derive(Clone, Default, Serialize, Deserialize)]
pub struct UefiVar {
    pub name: String,
    pub vendor_guid: String,

    pub attributes: VarAttributes,

    pub data_len: u64,
    pub data: Box<[u8]>,
}