use std::{convert::TryInto, num::NonZeroU32};

use automerge_protocol as amp;

use super::{
    focus::Focus, random_op_id, DiffApplicationResult, LocalOperationResult, MultiGrapheme,
    MultiValue, NewValueRequest, StateTree, StateTreeChange, StateTreeComposite, StateTreeList,
    StateTreeMap, StateTreeTable, StateTreeText, StateTreeValue,
};
use crate::{error, Cursor, Primitive, Value};

#[derive(Debug)]
pub struct ResolvedPath<'a> {
    pub(crate) target: Target<'a>,
}

pub enum Target<'a> {
    Root(ResolvedRoot<'a>),
    Map(ResolvedMap<'a>),
    Table(ResolvedTable<'a>),
    List(ResolvedList<'a>),
    Text(ResolvedText<'a>),
    Character(ResolvedChar<'a>),
    Counter(ResolvedCounter<'a>),
    Primitive(ResolvedPrimitive<'a>),
}

impl<'a> std::fmt::Debug for Target<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Target::Map(maptarget) => write!(f, "MapTarget {:?}", maptarget.value.object_id),
            Target::Root(_) => write!(f, "Root"),
            Target::Table(tabletarget) => {
                write!(f, "Table {:?}", tabletarget.value.object_id)
            }
            Target::List(listtarget) => write!(f, "list {:?}", listtarget.value.object_id),
            Target::Text(texttarget) => write!(f, "text {:?}", texttarget.value.object_id),
            Target::Counter(countertarget) => write!(
                f,
                "counter {0}:{1:?}",
                countertarget.containing_object_id, countertarget.key_in_container
            ),
            Target::Primitive(p) => write!(f, "primitive: {:?}", p.multivalue),
            Target::Character(ctarget) => write!(f, "character {:?}", ctarget.multivalue),
        }
    }
}

impl<'a> ResolvedPath<'a> {
    pub(super) fn new_root(tree: &mut StateTree) -> ResolvedPath {
        ResolvedPath {
            target: Target::Root(ResolvedRoot { root: tree }),
        }
    }

    pub(super) fn new_map(focus: Focus, map: &'a mut StateTreeMap) -> ResolvedPath<'a> {
        ResolvedPath {
            target: Target::Map(ResolvedMap { value: map, focus }),
        }
    }

    pub(super) fn new_list(
        mv: MultiValue,
        focus: Focus,
        list: &'a mut StateTreeList,
    ) -> ResolvedPath<'a> {
        ResolvedPath {
            target: Target::List(ResolvedList {
                multivalue: mv,
                focus,
                value: list,
            }),
        }
    }

    pub(super) fn new_text(
        mv: MultiValue,
        update: Box<dyn Fn(DiffApplicationResult<MultiValue>) -> StateTree>,
        text: &'a mut StateTreeText,
    ) -> ResolvedPath<'a> {
        ResolvedPath {
            target: Target::Text(ResolvedText {
                multivalue: mv,
                value: text,
                update,
            }),
        }
    }

    pub(super) fn new_table(
        mv: MultiValue,
        focus: Focus,
        table: &'a mut StateTreeTable,
    ) -> ResolvedPath<'a> {
        ResolvedPath {
            target: Target::Table(ResolvedTable {
                multivalue: mv,
                focus,
                value: table,
            }),
        }
    }

    pub(super) fn new_counter(
        object_id: amp::ObjectId,
        key: amp::Key,
        mv: &'a mut MultiValue,
        focus: Focus,
        value: i64,
    ) -> ResolvedPath<'a> {
        ResolvedPath {
            target: Target::Counter(ResolvedCounter {
                multivalue: mv,
                key_in_container: key,
                containing_object_id: object_id,
                current_value: value,
                focus,
            }),
        }
    }

    pub(super) fn new_primitive(value: &'a mut MultiValue) -> ResolvedPath<'a> {
        ResolvedPath {
            target: Target::Primitive(ResolvedPrimitive { multivalue: value }),
        }
    }

    pub(super) fn new_character(c: &'a mut MultiValue) -> ResolvedPath<'a> {
        ResolvedPath {
            target: Target::Character(ResolvedChar { multivalue: c }),
        }
    }

    pub fn default_value(&self) -> Value {
        match &self.target {
            Target::Map(maptarget) => maptarget.multivalue.default_value(),
            Target::Root(root) => root.root.value(),
            Target::Table(tabletarget) => tabletarget.multivalue.default_value(),
            Target::List(listtarget) => listtarget.multivalue.default_value(),
            Target::Text(texttarget) => texttarget.multivalue.default_value(),
            Target::Counter(countertarget) => countertarget.multivalue.default_value(),
            Target::Primitive(p) => p.multivalue.default_value(),
            Target::Character(ctarget) => ctarget.multivalue.default_value(),
        }
    }

    pub fn values(&self) -> std::collections::HashMap<amp::OpId, Value> {
        match &self.target {
            Target::Map(maptarget) => maptarget.multivalue.realise_values(),
            Target::Root(root) => {
                let mut result = std::collections::HashMap::new();
                result.insert(random_op_id(), root.root.value());
                result
            }
            Target::Table(tabletarget) => tabletarget.multivalue.realise_values(),
            Target::List(listtarget) => listtarget.multivalue.realise_values(),
            Target::Text(texttarget) => texttarget.multivalue.realise_values(),
            Target::Counter(countertarget) => countertarget.multivalue.realise_values(),
            Target::Primitive(p) => p.multivalue.realise_values(),
            Target::Character(ctarget) => ctarget.multivalue.realise_values(),
        }
    }

    pub fn object_id(&self) -> Option<amp::ObjectId> {
        match &self.target {
            Target::Map(maptarget) => Some(maptarget.value.object_id.clone()),
            Target::Root(_) => Some(amp::ObjectId::Root),
            Target::Table(tabletarget) => Some(tabletarget.value.object_id.clone()),
            Target::List(listtarget) => Some(listtarget.value.object_id.clone()),
            Target::Text(texttarget) => Some(texttarget.value.object_id.clone()),
            Target::Counter(_) => None,
            Target::Primitive(_) => None,
            Target::Character(_) => None,
        }
    }
}

pub(crate) struct SetOrInsertPayload<'a, T> {
    pub start_op: u64,
    pub actor: &'a amp::ActorId,
    pub value: T,
}

pub struct ResolvedRoot<'a> {
    pub(super) root: &'a mut StateTree,
}

impl<'a> ResolvedRoot<'a> {
    pub(crate) fn set_key(
        &mut self,
        key: &str,
        payload: SetOrInsertPayload<&Value>,
    ) -> LocalOperationResult {
        let newvalue = MultiValue::new_from_value_2(NewValueRequest {
            actor: payload.actor,
            start_op: payload.start_op,
            key: &key.into(),
            parent_obj: &amp::ObjectId::Root,
            value: payload.value,
            insert: false,
            pred: self
                .root
                .get(key)
                .map(|mv| vec![mv.default_opid()])
                .unwrap_or_else(Vec::new),
        });
        self.root
            .root_props
            .insert(key.to_string(), newvalue.multivalue());
        LocalOperationResult {
            new_ops: newvalue.ops(),
        }
    }

    pub(crate) fn delete_key(&self, key: &str) -> LocalOperationResult {
        let existing_value = self.root.get(key);
        let pred = existing_value
            .map(|v| vec![v.default_opid()])
            .unwrap_or_else(Vec::new);
        self.root.remove(key);
        LocalOperationResult {
            new_ops: vec![amp::Op {
                action: amp::OpType::Del(NonZeroU32::new(1).unwrap()),
                obj: amp::ObjectId::Root,
                key: key.into(),
                insert: false,
                pred,
            }],
        }
    }
}

pub struct ResolvedCounter<'a> {
    pub(super) current_value: i64,
    pub(super) multivalue: &'a mut MultiValue,
    pub(super) containing_object_id: amp::ObjectId,
    pub(super) key_in_container: amp::Key,
    pub(super) focus: Focus,
}

impl<'a> ResolvedCounter<'a> {
    pub(crate) fn increment(&self, by: i64) -> LocalOperationResult {
        let diffapp = DiffApplicationResult::pure(self.multivalue.update_default(
            StateTreeValue::Leaf(Primitive::Counter(self.current_value + by)),
        ));
        LocalOperationResult {
            new_ops: vec![amp::Op {
                action: amp::OpType::Inc(by),
                obj: self.containing_object_id.clone(),
                key: self.key_in_container.clone(),
                insert: false,
                pred: vec![self.multivalue.default_opid()],
            }],
        }
    }
}

pub struct ResolvedMap<'a> {
    pub(super) value: &'a mut StateTreeMap,
    pub(super) focus: Focus,
}

impl<'a> ResolvedMap<'a> {
    pub(crate) fn set_key(
        &mut self,
        key: &str,
        payload: SetOrInsertPayload<&Value>,
    ) -> LocalOperationResult {
        let newvalue = MultiValue::new_from_value_2(NewValueRequest {
            actor: payload.actor,
            start_op: payload.start_op,
            parent_obj: &self.value.object_id,
            key: &key.into(),
            value: payload.value,
            insert: false,
            pred: self.value.pred_for_key(key),
        });
        self.value
            .props
            .insert(key.to_string(), newvalue.multivalue());
        let diffapp = newvalue.diff_app_result().and_then(|v| {
            let new_value = self.value.update(key.to_string(), v);
            let new_composite = StateTreeComposite::Map(new_value);
            let new_mv = self
                .multivalue
                .update_default(StateTreeValue::Composite(new_composite));
            DiffApplicationResult::pure(new_mv).with_changes(StateTreeChange::single(
                self.value.object_id.clone(),
                new_composite,
            ))
        });
        LocalOperationResult {
            new_ops: newvalue.ops(),
        }
    }

    pub(crate) fn delete_key(&self, key: &str) -> LocalOperationResult {
        let new_value = self.value.without(key);
        let new_composite = StateTreeComposite::Map(new_value);
        let new_mv = self
            .multivalue
            .update_default(StateTreeValue::Composite(new_composite));
        let diffapp = DiffApplicationResult::pure(new_mv).with_changes(StateTreeChange::single(
            new_composite.object_id(),
            new_composite,
        ));
        LocalOperationResult {
            new_ops: vec![amp::Op {
                action: amp::OpType::Del(NonZeroU32::new(1).unwrap()),
                obj: self.value.object_id.clone(),
                key: key.into(),
                insert: false,
                pred: self.value.pred_for_key(key),
            }],
        }
    }
}

pub struct ResolvedTable<'a> {
    pub(super) value: &'a mut StateTreeTable,
    pub(super) multivalue: MultiValue,
    pub(super) focus: Focus,
}

impl<'a> ResolvedTable<'a> {
    pub(crate) fn set_key(
        &self,
        key: &str,
        payload: SetOrInsertPayload<&Value>,
    ) -> LocalOperationResult {
        let newvalue = MultiValue::new_from_value_2(NewValueRequest {
            actor: payload.actor,
            start_op: payload.start_op,
            parent_obj: &self.value.object_id,
            key: &key.into(),
            value: payload.value,
            insert: false,
            pred: self.value.pred_for_key(key),
        });
        let treechange = newvalue.diff_app_result().and_then(|v| {
            let new_value = self.value.update(key.to_string(), v);
            let new_composite = StateTreeComposite::Table(new_value);
            let new_mv = self
                .multivalue
                .update_default(StateTreeValue::Composite(new_composite));
            DiffApplicationResult::pure(new_mv).with_changes(StateTreeChange::single(
                self.value.object_id.clone(),
                new_composite,
            ))
        });
        LocalOperationResult {
            new_ops: newvalue.ops(),
        }
    }

    pub(crate) fn delete_key(&self, key: &str) -> LocalOperationResult {
        let new_value = self.value.without(key);
        let new_composite = StateTreeComposite::Table(new_value);
        let new_mv = self
            .multivalue
            .update_default(StateTreeValue::Composite(new_composite));
        let diffapp = DiffApplicationResult::pure(new_mv).with_changes(StateTreeChange::single(
            new_composite.object_id(),
            new_composite,
        ));
        LocalOperationResult {
            new_ops: vec![amp::Op {
                action: amp::OpType::Del(NonZeroU32::new(1).unwrap()),
                obj: self.value.object_id.clone(),
                key: key.into(),
                insert: false,
                pred: self.value.pred_for_key(key),
            }],
        }
    }
}

pub struct ResolvedText<'a> {
    pub(super) value: &'a mut StateTreeText,
    pub(super) multivalue: MultiValue,
    pub(super) update: Box<dyn Fn(DiffApplicationResult<MultiValue>) -> StateTree>,
}

impl<'a> ResolvedText<'a> {
    #[allow(dead_code)]
    pub(crate) fn insert(
        &self,
        index: u32,
        payload: SetOrInsertPayload<String>,
    ) -> Result<LocalOperationResult, error::MissingIndexError> {
        let current_elemid = match index {
            0 => amp::ElementId::Head,
            i => self.value.elem_at((i - 1).try_into().unwrap())?.0.into(),
        };
        let insert_op = amp::OpId::new(payload.start_op, payload.actor);
        let c = MultiGrapheme::new_from_grapheme_cluster(insert_op, payload.value.clone());
        let new_text = self.value.insert(index.try_into().unwrap(), c)?;
        let updated = StateTreeComposite::Text(new_text);
        let mv = self
            .multivalue
            .update_default(StateTreeValue::Composite(updated));
        let treechange = DiffApplicationResult::pure(mv).with_changes(StateTreeChange::single(
            self.value.object_id.clone(),
            updated,
        ));
        Ok(LocalOperationResult {
            new_ops: vec![amp::Op {
                action: amp::OpType::Set(amp::ScalarValue::Str(payload.value)),
                obj: self.value.object_id.clone(),
                key: current_elemid.into(),
                insert: true,
                pred: Vec::new(),
            }],
        })
    }

    pub(crate) fn insert_many<I>(
        &self,
        index: u32,
        payload: SetOrInsertPayload<I>,
    ) -> Result<LocalOperationResult, error::MissingIndexError>
    where
        I: ExactSizeIterator<Item = String>,
    {
        let current_elemid = match index {
            0 => amp::ElementId::Head,
            i => self.value.elem_at((i - 1).try_into().unwrap())?.0.into(),
        };
        let mut values = Vec::with_capacity(payload.value.len());
        let mut chars: Vec<amp::ScalarValue> = Vec::with_capacity(payload.value.len());
        for (i, c) in payload.value.enumerate() {
            let insert_op = amp::OpId::new(payload.start_op + i as u64, payload.actor);
            chars.push(amp::ScalarValue::Str(c.to_string()));
            let c = MultiGrapheme::new_from_grapheme_cluster(insert_op, c);
            values.push(c)
        }
        let new_text = self.value.insert_many(index.try_into().unwrap(), values)?;
        let updated = StateTreeComposite::Text(new_text);
        let mv = self
            .multivalue
            .update_default(StateTreeValue::Composite(updated));
        let treechange = DiffApplicationResult::pure(mv).with_changes(StateTreeChange::single(
            self.value.object_id.clone(),
            updated,
        ));
        let action = match chars.len() {
            1 => amp::OpType::Set(chars[0].clone()),
            _ => amp::OpType::MultiSet(chars),
        };
        Ok(LocalOperationResult {
            new_ops: vec![amp::Op {
                action,
                obj: self.value.object_id.clone(),
                key: current_elemid.into(),
                insert: true,
                pred: Vec::new(),
            }],
        })
    }

    pub(crate) fn set(
        &self,
        index: u32,
        payload: SetOrInsertPayload<String>,
    ) -> Result<LocalOperationResult, error::MissingIndexError> {
        let index: usize = index.try_into().unwrap();
        let (current_elemid, _) = self.value.elem_at(index)?;
        let update_op = amp::OpId::new(payload.start_op, payload.actor);
        let c = MultiGrapheme::new_from_grapheme_cluster(update_op, payload.value.clone());
        let updated = StateTreeComposite::Text(self.value.set(index, c)?);
        let mv = self
            .multivalue
            .update_default(StateTreeValue::Composite(updated));
        let diffapp = DiffApplicationResult::pure(mv).with_changes(StateTreeChange::single(
            self.value.object_id.clone(),
            updated,
        ));
        Ok(LocalOperationResult {
            new_ops: vec![amp::Op {
                action: amp::OpType::Set(amp::ScalarValue::Str(payload.value)),
                obj: self.value.object_id.clone(),
                key: current_elemid.into(),
                pred: self.value.pred_for_index(index as u32),
                insert: false,
            }],
        })
    }

    pub(crate) fn remove(
        &self,
        index: u32,
    ) -> Result<LocalOperationResult, error::MissingIndexError> {
        let (current_elemid, _) = self.value.elem_at(index.try_into().unwrap())?;
        let updated = StateTreeComposite::Text(self.value.remove(index.try_into().unwrap())?);
        let mv = self
            .multivalue
            .update_default(StateTreeValue::Composite(updated));
        let diffapp = DiffApplicationResult::pure(mv).with_changes(StateTreeChange::single(
            self.value.object_id.clone(),
            updated,
        ));
        Ok(LocalOperationResult {
            new_ops: vec![amp::Op {
                action: amp::OpType::Del(NonZeroU32::new(1).unwrap()),
                obj: self.value.object_id.clone(),
                key: current_elemid.into(),
                insert: false,
                pred: self.value.pred_for_index(index as u32),
            }],
        })
    }

    pub(crate) fn get_cursor(&self, index: u32) -> Result<Cursor, error::MissingIndexError> {
        let (current_elemid, _) = self.value.elem_at(index.try_into().unwrap())?;
        Ok(Cursor::new(
            index,
            self.value.object_id.clone(),
            current_elemid.clone(),
        ))
    }
}

pub struct ResolvedList<'a> {
    pub(super) value: &'a mut StateTreeList,
    pub(super) multivalue: MultiValue,
    pub(super) focus: Focus,
}

impl<'a> ResolvedList<'a> {
    pub(crate) fn set(
        &self,
        index: u32,
        payload: SetOrInsertPayload<&Value>,
    ) -> Result<LocalOperationResult, error::MissingIndexError> {
        let (current_elemid, _) = self.value.elem_at(index.try_into().unwrap())?;
        let newvalue = MultiValue::new_from_value_2(NewValueRequest {
            actor: payload.actor,
            start_op: payload.start_op,
            value: payload.value,
            pred: self.value.pred_for_index(index),
            parent_obj: &self.value.object_id.clone(),
            key: &current_elemid.into(),
            insert: false,
        });
        let treechange = newvalue.diff_app_result().try_and_then(|v| {
            let new_value = StateTreeComposite::List(self.value.set(index.try_into().unwrap(), v)?);
            let mv = self
                .multivalue
                .update_default(StateTreeValue::Composite(new_value));
            Ok(
                DiffApplicationResult::pure(mv).with_changes(StateTreeChange::single(
                    self.value.object_id.clone(),
                    new_value,
                )),
            )
        })?;
        Ok(LocalOperationResult {
            new_ops: newvalue.ops(),
        })
    }

    #[allow(dead_code)]
    pub(crate) fn insert(
        &self,
        index: u32,
        payload: SetOrInsertPayload<&Value>,
    ) -> Result<LocalOperationResult, error::MissingIndexError> {
        let current_elemid = match index {
            0 => amp::ElementId::Head,
            i => self
                .value
                .elem_at((i - 1).try_into().unwrap())?
                .0
                .clone()
                .into(),
        };
        let newvalue = MultiValue::new_from_value_2(NewValueRequest {
            actor: payload.actor,
            start_op: payload.start_op,
            value: payload.value,
            parent_obj: &self.value.object_id,
            key: &current_elemid.into(),
            insert: true,
            pred: Vec::new(),
        });
        let treechange = newvalue.diff_app_result().try_and_then(|v| {
            let new_value =
                StateTreeComposite::List(self.value.insert(index.try_into().unwrap(), v)?);
            let mv = self
                .multivalue
                .update_default(StateTreeValue::Composite(new_value));
            Ok(
                DiffApplicationResult::pure(mv).with_changes(StateTreeChange::single(
                    self.value.object_id.clone(),
                    new_value,
                )),
            )
        })?;
        Ok(LocalOperationResult {
            new_ops: newvalue.ops(),
        })
    }

    pub(crate) fn insert_many<'b, I>(
        &'a self,
        index: u32,
        payload: SetOrInsertPayload<I>,
    ) -> Result<LocalOperationResult, error::MissingIndexError>
    where
        I: ExactSizeIterator<Item = &'b Value>,
    {
        let mut last_elemid = match index {
            0 => amp::ElementId::Head,
            i => self
                .value
                .elem_at((i - 1).try_into().unwrap())?
                .0
                .clone()
                .into(),
        };
        let mut newvalues = DiffApplicationResult::pure(Vec::with_capacity(payload.value.len()));
        let mut op_num = payload.start_op;
        let mut ops = Vec::new();
        for value in payload.value {
            let newvalue = MultiValue::new_from_value_2(NewValueRequest {
                actor: payload.actor,
                start_op: op_num,
                value: &value,
                parent_obj: &self.value.object_id,
                key: &last_elemid.clone().into(),
                insert: true,
                pred: Vec::new(),
            });
            last_elemid = amp::OpId::new(op_num, payload.actor).into();
            op_num = newvalue.max_op() + 1;
            newvalues = newvalue.diff_app_result().and_then(|v| {
                newvalues.map(|mut vs| {
                    vs.push(v);
                    vs
                })
            });
            ops.extend(newvalue.ops());
        }
        let treechange = newvalues.try_and_then(|vs| {
            let new_value =
                StateTreeComposite::List(self.value.insert_many(index.try_into().unwrap(), vs)?);
            let mv = self
                .multivalue
                .update_default(StateTreeValue::Composite(new_value));
            Ok(
                DiffApplicationResult::pure(mv).with_changes(StateTreeChange::single(
                    self.value.object_id.clone(),
                    new_value,
                )),
            )
        })?;
        Ok(LocalOperationResult {
            new_ops: condense_insert_ops(ops),
        })
    }

    pub(crate) fn remove(
        &self,
        index: u32,
    ) -> Result<LocalOperationResult, error::MissingIndexError> {
        let (current_elemid, _) = self.value.elem_at(index.try_into().unwrap())?;
        let new_value = StateTreeComposite::List(self.value.remove(index.try_into().unwrap())?);
        let mv = self
            .multivalue
            .update_default(StateTreeValue::Composite(new_value));
        let treechange = DiffApplicationResult::pure(mv).with_changes(StateTreeChange::single(
            self.value.object_id.clone(),
            new_value,
        ));
        Ok(LocalOperationResult {
            new_ops: vec![amp::Op {
                action: amp::OpType::Del(NonZeroU32::new(1).unwrap()),
                obj: self.value.object_id.clone(),
                key: current_elemid.into(),
                insert: false,
                pred: self.value.pred_for_index(index),
            }],
        })
    }

    pub(crate) fn get_cursor(&self, index: u32) -> Result<Cursor, error::MissingIndexError> {
        let (current_elemid, _) = self.value.elem_at(index.try_into().unwrap())?;
        Ok(Cursor::new(
            index,
            self.value.object_id.clone(),
            current_elemid.clone(),
        ))
    }
}

pub struct ResolvedChar<'a> {
    pub(super) multivalue: &'a mut MultiValue,
}

pub struct ResolvedPrimitive<'a> {
    pub(super) multivalue: &'a mut MultiValue,
}

fn condense_insert_ops(ops: Vec<amp::Op>) -> Vec<amp::Op> {
    if ops.len() == 1 {
        return ops;
    }
    let mut op_iter = ops.iter();
    let mut prim_vals = Vec::with_capacity(ops.len());
    let mut preds = Vec::new();
    if let Some(v) = op_iter.next() {
        if let Some(prim) = prim_from_op_action(&v.action) {
            prim_vals.push(prim);
            preds.extend(v.pred.clone());
        }
        for o in op_iter {
            if let Some(prim) = prim_from_op_action(&o.action) {
                prim_vals.push(prim);
                preds.extend(o.pred.clone());
            }
        }
        if prim_vals.len() == ops.len() {
            vec![amp::Op {
                action: amp::OpType::MultiSet(prim_vals),
                pred: preds,
                insert: true,
                key: v.key.clone(),
                obj: v.obj.clone(),
            }]
        } else {
            ops
        }
    } else {
        ops
    }
}

fn prim_from_op_action(action: &amp::OpType) -> Option<amp::ScalarValue> {
    match action {
        amp::OpType::Set(v) => match v {
            amp::ScalarValue::Bytes(_) => Some(v.clone()),
            amp::ScalarValue::Str(_) => Some(v.clone()),
            amp::ScalarValue::Int(_) => Some(v.clone()),
            amp::ScalarValue::Uint(_) => Some(v.clone()),
            amp::ScalarValue::F64(_) => Some(v.clone()),
            amp::ScalarValue::F32(_) => Some(v.clone()),
            amp::ScalarValue::Counter(_) => None,
            amp::ScalarValue::Timestamp(_) => None,
            amp::ScalarValue::Cursor(_) => None,
            amp::ScalarValue::Boolean(_) => Some(v.clone()),
            amp::ScalarValue::Null => Some(v.clone()),
        },
        _ => None,
    }
}
