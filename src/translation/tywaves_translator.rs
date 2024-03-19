use std::fmt::Debug;
use std::os::unix::raw;
use std::sync::mpsc::Sender;

use camino::Utf8Path;
use ron::value;
use spade_mir::value_name;

use crate::message::Message;
use crate::translation::{Translator, ValueKind, ValueRepr};
use crate::wasm_util::perform_work;
use crate::wave_container::{VariableMeta, VariableValue};

use tywaves::symbol_table::{self, RealType};
use tywaves::tywaves_state::TywaveState;
use tywaves::variable_finder::VariableFinder;

use super::{TranslationPreference, TranslationResult, VariableInfo};

use color_eyre::{eyre::eyre, Result}; // Add this line

// TODO: implement the conversion from tywaves types to VariableInfo
impl From<&symbol_table::RealType> for VariableInfo {
    fn from(real_type: &symbol_table::RealType) -> Self {
        match real_type {
            symbol_table::RealType::Ground { width, .. } => {
                if *width == 1 {
                    VariableInfo::Bool
                } else {
                    VariableInfo::Bits
                }
            }
            symbol_table::RealType::Bundle { fields, vcd_name } => VariableInfo::Compound {
                subfields: fields
                    .iter()
                    .map(|f| (f.name.clone(), Self::from(&f.real_type)))
                    .collect(),
            },
            _ => VariableInfo::String,
        }
    }
}

pub struct TywavesTranslator {
    state: TywaveState,
}

impl TywavesTranslator {
    pub fn new(state_file: &Utf8Path) -> Result<Self> {
        // Parse the state file using serde and initialize the TywavesTranslator
        let state_file = std::fs::read_to_string(state_file)?;
        let state = serde_json::from_str(&state_file)?;
        Ok(TywavesTranslator { state })
    }

    pub fn load(state_file: &Utf8Path, sender: Sender<Message>) {
        let state_file = state_file.to_owned();

        perform_work(move || {
            let t = TywavesTranslator::new(&state_file);
            match t {
                Ok(result) => sender
                    .send(Message::TranslatorLoaded(Box::new(result)))
                    .unwrap(),
                Err(e) => sender.send(Message::Error(e)).unwrap(),
            }
        });
    }

    /// Given a variable meta, this explores the state and finds the variable
    fn find_variable(&self, variable: &VariableMeta) -> Result<&symbol_table::Variable> {
        self.state
            .find_variable(&variable.var.full_path())
            .ok_or_else(|| {
                eyre!(
                    "Variable not found in the state: {:?}",
                    variable.var.full_path()
                )
            })
    }

    fn find_parent_variable(&self, variable: &VariableMeta) -> Result<&symbol_table::Variable> {
        let path = variable.var.full_path();
        self.state
            .find_parent_variable(&path)
            .ok_or(eyre!("Parent variable not found in the state: {:?}", path))
    }

    fn get_sub_raw_val<'a>(
        &self,
        subfield_t: &RealType,
        raw_val_vcd: &'a str,
    ) -> (&'a str, &'a str) {
        // Get size of real type
        let size = subfield_t.find_width() as usize;
        if raw_val_vcd.len() < size {
            return ("0", raw_val_vcd);
        }
        (&raw_val_vcd[..size], &raw_val_vcd[size..])
    }

    fn translate_realtype(
        &self,
        variable: &symbol_table::Variable,
        raw_val_vcd: &str,
    ) -> Result<TranslationResult> {

        let real_type = &variable.real_type;
        let val = ValueRepr::String(variable.create_val_repr(raw_val_vcd));
        let result = match real_type {
            symbol_table::RealType::Ground { .. } => {
                println!("val: {:?}", val);
                let subfields = vec![];
                let kind = ValueKind::Normal; // TODO!: change this to use the correct value
                TranslationResult {
                    val,
                    subfields,
                    kind,
                }
            }
            symbol_table::RealType::Bundle { fields, .. } => {
                // Collect the subfields of the bundle
                let mut subfields = vec![];

                let mut _raw_val_vcd = raw_val_vcd;
                let mut _val = "0";
                for field in fields {
                    // Get the value of the subfield
                    (_val, _raw_val_vcd) = self.get_sub_raw_val(&field.real_type, _raw_val_vcd);
                    println!("val: {}, raw_val_vcd: {}", _val, _raw_val_vcd);
                    let value_repr = ValueRepr::String(_val.to_string());
                    let subfield = TranslationResult {
                        val: value_repr,
                        subfields: vec![],
                        kind: ValueKind::Normal,
                    };
                    subfields.push(super::SubFieldTranslationResult::new(
                        field.name.clone(),
                        subfield,
                    ));
                }
                TranslationResult {
                    val,
                    subfields,
                    kind: ValueKind::Normal,
                }
            }
            symbol_table::RealType::Vec { .. } => {
                // let value_repr = ValueRepr::String(raw_val_vcd.to_string());
                // let subfields = vec![];
                // let kind = ValueKind::Normal;
                // TranslationResult {
                //     val: value_repr,
                //     subfields,
                //     kind,
                // }
                todo!("Vec type not implemented")
            }
            _ => TranslationResult {
                val: ValueRepr::String(raw_val_vcd.to_string()),
                subfields: vec![],
                kind: ValueKind::Undef,
            },
        };
        Ok(result)
        // Err(eyre!("Not implemented"))
    }
}

impl Translator for TywavesTranslator {
    fn name(&self) -> String {
        "Tywaves".to_string()
    }

    fn translate(
        &self,
        variable: &VariableMeta,
        value: &VariableValue,
    ) -> Result<TranslationResult> {
        let var = self.find_variable(variable)?;
        // println!("Variable: {:?}", var);
        let parent_var = self.find_parent_variable(variable)?;
        // println!("Parent Variable: {:?}", parent_var);

        // Extract the raw value from the vcd in binary format
        let raw_val_vcd = match value {
            VariableValue::BigUint(v) => format!("{v:0b}"),
            VariableValue::String(v) => v.clone(),
        };

        // Add extra bits 
        let raw_val_vcd = format!("{:0>width$}", raw_val_vcd, width = var.real_type.find_width() as usize);

        // let value_repr: ValueRepr = ValueRepr::String(&raw_val_vcd);
        // let subfields = vec![];
        // let kind = ValueKind::Normal;
        // let result = TranslationResult {
        //     val: value_repr,
        //     subfields,
        //     kind,
        // };

        let parent_var = var;
        self.translate_realtype(&parent_var, &raw_val_vcd)
        // Err(eyre!("Not implemented"))
    }

    fn variable_info(&self, variable: &VariableMeta) -> Result<VariableInfo> {
        let var = self.find_variable(variable)?;
        println!("Variable: {:?}", var);
        let parent_var = self.find_parent_variable(variable)?;
        // println!("Parent Variable: {:?}", parent_var);
        let parent_var = var;
        // Convert parent var to VariableInfo
        let parent_var_info = VariableInfo::from(&parent_var.real_type);
        // println!("Parent Variable Info: {:?}", parent_var_info);
        Ok(parent_var_info)
        // todo!("VariableInfo to implement")
    }

    // Tell if the variable can be translated or not
    fn translates(&self, variable: &VariableMeta) -> Result<TranslationPreference> {
        // Get the type of the selected variable
        if let Ok(_) = &self.find_variable(variable) {
            Ok(TranslationPreference::Prefer)
        } else {
            Ok(TranslationPreference::No)
        }

        // todo!()
    }
}
