use amp::OpId;
use automerge_protocol as amp;

use super::{MultiGrapheme, MultiValue, StateTreeChange};
use crate::error::InvalidPatch;

pub(super) trait DiffableValue: Sized {
    fn construct(opid: &amp::OpId, diff: &amp::Diff) -> Result<Self, InvalidPatch>;

    fn apply_diff(&mut self, opid: &amp::OpId, diff: &amp::Diff) -> Result<(), InvalidPatch>;

    fn apply_diff_iter<'a, 'b, 'c, 'd, I>(&'a mut self, diff: &mut I) -> Result<(), InvalidPatch>
    where
        I: Iterator<Item = (&'b amp::OpId, &'d amp::Diff)>;

    fn default_opid(&self) -> amp::OpId;

    fn only_for_opid(&self, opid: &amp::OpId) -> Option<Self>;

    fn add_values_from(&mut self, other: Self);
}

impl DiffableValue for MultiGrapheme {
    fn construct(opid: &amp::OpId, diff: &amp::Diff) -> Result<Self, InvalidPatch> {
        let c = MultiGrapheme::new_from_diff(opid, diff)?;
        Ok(c)
    }

    fn apply_diff(&mut self, opid: &amp::OpId, diff: &amp::Diff) -> Result<(), InvalidPatch> {
        MultiGrapheme::apply_diff(self, opid, diff)
    }

    fn apply_diff_iter<'a, 'b, 'c, 'd, I>(&'a mut self, diff: &mut I) -> Result<(), InvalidPatch>
    where
        I: Iterator<Item = (&'b amp::OpId, &'d amp::Diff)>,
    {
        self.apply_diff_iter(diff)
        //MultiGrapheme::apply_diff_iter(self, diff)
    }

    fn default_opid(&self) -> amp::OpId {
        self.default_opid().clone()
    }

    fn only_for_opid(&self, opid: &amp::OpId) -> Option<MultiGrapheme> {
        self.only_for_opid(opid)
    }

    fn add_values_from(&mut self, other: MultiGrapheme) {
        self.add_values_from(other)
    }
}

impl DiffableValue for MultiValue {
    fn construct(opid: &amp::OpId, diff: &amp::Diff) -> Result<Self, InvalidPatch> {
        MultiValue::new_from_diff(opid.clone(), diff)
    }

    fn apply_diff(&mut self, opid: &amp::OpId, diff: &amp::Diff) -> Result<(), InvalidPatch> {
        self.apply_diff(opid, diff)
    }

    fn apply_diff_iter<'a, 'b, 'c, 'd, I>(&'a mut self, diff: &mut I) -> Result<(), InvalidPatch>
    where
        I: Iterator<Item = (&'b amp::OpId, &'d amp::Diff)>,
    {
        self.apply_diff_iter(diff)
    }

    fn default_opid(&self) -> amp::OpId {
        self.default_opid()
    }

    fn only_for_opid(&self, opid: &amp::OpId) -> Option<MultiValue> {
        self.only_for_opid(opid)
    }

    fn add_values_from(&mut self, other: MultiValue) {
        self.add_values_from(other)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct DiffableSequence<T>
where
    T: DiffableValue,
    T: Clone,
    T: PartialEq,
{
    // stores the opid that created the element and the diffable value
    underlying: Box<im_rc::Vector<(OpId, UpdatingSequenceElement<T>)>>,
}

impl<T> DiffableSequence<T>
where
    T: Clone,
    T: DiffableValue,
    T: PartialEq,
{
    pub fn new() -> DiffableSequence<T> {
        DiffableSequence {
            underlying: Box::new(im_rc::Vector::new()),
        }
    }

    pub(super) fn new_from<I>(i: I) -> DiffableSequence<T>
    where
        I: IntoIterator<Item = T>,
    {
        DiffableSequence {
            underlying: Box::new(
                i.into_iter()
                    .map(|i| (i.default_opid(), UpdatingSequenceElement::Original(i)))
                    .collect(),
            ),
        }
    }

    pub fn apply_diff(
        &mut self,
        object_id: &amp::ObjectId,
        edits: &[amp::DiffEdit],
    ) -> Result<(), InvalidPatch> {
        let mut changes = StateTreeChange::empty();
        for edit in edits.iter() {
            match edit {
                amp::DiffEdit::Remove { index, count } => {
                    let index = *index as usize;
                    let count = *count as usize;
                    if index >= self.underlying.len() {
                        return Err(InvalidPatch::InvalidIndex {
                            object_id: object_id.clone(),
                            index,
                        });
                    }
                    if index + count > self.underlying.len() {
                        return Err(InvalidPatch::InvalidIndex {
                            object_id: object_id.clone(),
                            index: self.underlying.len(),
                        });
                    }
                    for i in index..(index + count) {
                        self.underlying.remove(i);
                    }
                }
                amp::DiffEdit::SingleElementInsert {
                    index,
                    elem_id: _,
                    op_id,
                    value,
                } => {
                    let node = T::construct(&op_id, value)?;
                    if (*index as usize) == self.underlying.len() {
                        self.underlying
                            .push_back((node.default_opid(), UpdatingSequenceElement::new(node)));
                    } else {
                        self.underlying.insert(
                            *index as usize,
                            (node.default_opid(), UpdatingSequenceElement::new(node)),
                        );
                    };
                }
                amp::DiffEdit::MultiElementInsert {
                    elem_id,
                    values,
                    index,
                } => {
                    let index = *index as usize;
                    if index > self.underlying.len() {
                        return Err(InvalidPatch::InvalidIndex {
                            index,
                            object_id: object_id.clone(),
                        });
                    }
                    for (i, value) in values.iter().enumerate() {
                        let opid = elem_id.as_opid().unwrap().increment_by(i as u64);
                        let mv = T::construct(&opid, &amp::Diff::Value(value.clone()))?;
                        self.underlying.insert(
                            index + i,
                            (mv.default_opid(), UpdatingSequenceElement::New(mv)),
                        );
                    }
                }
                amp::DiffEdit::Update {
                    index,
                    value,
                    op_id,
                } => {
                    if let Some((_id, elem)) = self.underlying.get_mut(*index as usize) {
                        elem.apply_diff(op_id, value)?;
                    } else {
                        return Err(InvalidPatch::InvalidIndex {
                            index: *index as usize,
                            object_id: object_id.clone(),
                        });
                    }
                }
            };
        }

        for element in self.underlying.iter_mut() {
            element.1.finish()
        }

        Ok(())
    }

    pub(super) fn remove(&mut self, index: usize) -> T {
        self.underlying.remove(index).1.get().clone()
    }

    pub(super) fn len(&self) -> usize {
        self.underlying.len()
    }

    pub(super) fn update(&self, index: usize, value: T) -> Self {
        let elem_id = if let Some(existing) = self.underlying.get(index) {
            existing.0.clone()
        } else {
            value.default_opid()
        };
        DiffableSequence {
            underlying: Box::new(
                self.underlying
                    .update(index, (elem_id, UpdatingSequenceElement::Original(value))),
            ),
        }
    }

    pub(super) fn get(&self, index: usize) -> Option<(&OpId, &T)> {
        self.underlying.get(index).map(|(i, t)| (i, t.get()))
    }

    pub(super) fn get_mut(&mut self, index: usize) -> Option<(&OpId, &mut T)> {
        self.underlying
            .get_mut(index)
            .map(|(i, t)| (&*i, t.get_mut()))
    }

    pub(super) fn insert(&mut self, index: usize, value: T) {
        self.underlying.insert(
            index,
            (
                value.default_opid(),
                UpdatingSequenceElement::Original(value),
            ),
        )
    }

    pub(super) fn mutate<F>(&mut self, index: usize, f: F)
    where
        F: FnOnce(&T) -> T,
    {
        if let Some(entry) = self.underlying.get_mut(index) {
            let t = entry.1.get();
            *entry = (entry.0.clone(), UpdatingSequenceElement::Original(f(&t)));
        }
    }

    pub(super) fn iter(&self) -> impl std::iter::Iterator<Item = &T> {
        // Making this unwrap safe is the entire point of this data structure
        self.underlying.iter().map(|i| i.1.get())
    }
}

#[derive(Clone, Debug, PartialEq)]
enum UpdatingSequenceElement<T>
where
    T: DiffableValue,
{
    Original(T),
    New(T),
    Updated {
        original: T,
        initial_update: T,
        remaining_updates: Vec<T>,
    },
}

impl<T> UpdatingSequenceElement<T>
where
    T: DiffableValue,
    T: Clone,
{
    fn new(value: T) -> UpdatingSequenceElement<T> {
        UpdatingSequenceElement::New(value)
    }

    fn finish(&mut self) {
        match self {
            UpdatingSequenceElement::Original(_) => { // do nothing, this is the finished state
            }
            UpdatingSequenceElement::New(v) => *self = UpdatingSequenceElement::Original(v.clone()),
            UpdatingSequenceElement::Updated {
                initial_update,
                remaining_updates,
                ..
            } => {
                let t = std::mem::take(remaining_updates).into_iter().fold(
                    initial_update.clone(),
                    |mut acc, elem| {
                        acc.add_values_from(elem);
                        acc
                    },
                );
                *self = UpdatingSequenceElement::Original(t)
            }
        }
    }

    fn get(&self) -> &T {
        match self {
            UpdatingSequenceElement::Original(v) => v,
            _ => unreachable!(),
        }
    }

    fn get_mut(&mut self) -> &mut T {
        match self {
            UpdatingSequenceElement::Original(v) => v,
            _ => unreachable!(),
        }
    }

    fn apply_diff(&mut self, opid: &amp::OpId, diff: &amp::Diff) -> Result<(), InvalidPatch> {
        match self {
            UpdatingSequenceElement::Original(v) => {
                let updated = if let Some(existing) = v.only_for_opid(opid) {
                    existing.apply_diff(opid, diff)?;
                    existing
                } else {
                    T::construct(opid, diff)?
                };
                *self = UpdatingSequenceElement::Updated {
                    original: v.clone(),
                    initial_update: updated,
                    remaining_updates: Vec::new(),
                };
                Ok(())
            }
            UpdatingSequenceElement::New(v) => {
                let updated = if let Some(existing) = v.only_for_opid(opid) {
                    existing.apply_diff(opid, diff)?;
                    existing
                } else {
                    T::construct(opid, diff)?
                };
                *self = UpdatingSequenceElement::Updated {
                    original: v.clone(),
                    initial_update: v.clone(),
                    remaining_updates: vec![updated],
                };
                Ok(())
            }
            UpdatingSequenceElement::Updated {
                original,
                initial_update,
                remaining_updates,
            } => {
                println!("Updating already updated value");
                let updated = if let Some(update) =
                    remaining_updates.iter().find_map(|v| v.only_for_opid(opid))
                {
                    update.apply_diff(opid, diff)?;
                    update
                } else if let Some(initial) = initial_update.only_for_opid(opid) {
                    initial.apply_diff(opid, diff)?;
                    initial
                } else if let Some(original) = original.only_for_opid(opid) {
                    original.apply_diff(opid, diff)?;
                    original
                } else {
                    T::construct(opid, diff)?
                };
                remaining_updates.push(updated);
                Ok(())
            }
        }
    }
}
