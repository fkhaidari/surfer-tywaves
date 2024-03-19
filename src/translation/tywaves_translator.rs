use std::sync::mpsc::Sender;

use camino::Utf8Path;

use crate::message::Message;
use crate::translation::Translator;
use crate::wasm_util::perform_work;
use crate::wave_container::{VariableMeta, VariableValue};

use tywaves::symbol_table;
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
            symbol_table::RealType::Bundle { fields } => VariableInfo::Compound {
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

  
        todo!("Translate to implement")
    }

    fn variable_info(&self, variable: &VariableMeta) -> Result<VariableInfo> {
        let var = self.find_variable(variable)?;
        println!("Variable: {:?}", var);
        let parent_var = self.find_parent_variable(variable)?;
        println!("Parent Variable: {:?}", parent_var);

        // Convert parent var to VariableInfo
        let parent_var_info = VariableInfo::from(&parent_var.real_type);
        println!("Parent Variable Info: {:?}", parent_var_info);
        Ok(parent_var_info)
        // todo!("VariableInfo to implement")
    }

    // Tell if the variable can be translated or not
    fn translates(&self, variable: &VariableMeta) -> Result<TranslationPreference> {
        // Get the type of the selected variable
        let var_from_symbol_table = &self.find_variable(variable);

        Ok(TranslationPreference::Prefer)
        // todo!()
    }
}
