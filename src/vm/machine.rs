use crate::{
    data_structure::{PStack, PVector},
    intrinsic,
    vm::{
        bytecode::{Closure, NamedFunction},
        error::QueryExecutionError,
        Address, ByteCode, Program, Result, ScopeId, ScopedSlot, Value,
    },
};
use itertools::Itertools;
use std::{
    cell::{RefCell, RefMut},
    rc::Rc,
};
use thiserror::Error;

#[derive(Debug, Clone, Eq, PartialEq, Error)]
pub(crate) enum ProgramError {
    #[error("tried to pop an empty stack")]
    PopEmptyStack,
    #[error("tried to use an uninitialized scope")]
    UninitializedScope,
    #[error("tried to load from an uninitialized slot of a scope")]
    UnknownSlot,
    #[error("tried to load from an uninitialized slot of a scope")]
    UninitializedSlot,
    #[error("tried to pop a scope but there was no scope to pop")]
    PopEmptyScope,
    #[error("tried to restore an unknown scope")]
    PopUnknownScope,
}

pub(crate) enum PathElement {
    Array(isize),
    Object(Rc<String>),
    Slice(Option<isize>, Option<isize>),
}

#[derive(Debug, Clone)]
enum PathValueIterator {
    Array {
        array: PVector<Value>,
        next: usize,
    },
    Object {
        sorted_elements: PVector<(Rc<String>, Value)>,
        next: usize,
    },
}

impl Iterator for PathValueIterator {
    type Item = (PathElement, Value);

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            PathValueIterator::Array { array, next } => {
                if *next >= array.len() {
                    None
                } else {
                    let ret = (PathElement::Array(*next as isize), array[*next].clone());
                    *next += 1;
                    Some(ret)
                }
            }
            PathValueIterator::Object {
                sorted_elements,
                next,
            } => {
                if *next >= sorted_elements.len() {
                    None
                } else {
                    let (index, value) = sorted_elements[*next].clone();
                    *next += 1;
                    Some((PathElement::Object(index), value))
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
struct Scope {
    slots: PVector<Rc<RefCell<Option<Value>>>>,
    closure_slots: PVector<Rc<RefCell<Option<Closure>>>>,
}

impl Scope {
    fn new(variable_cnt: usize, closure_cnt: usize) -> Self {
        Self {
            slots: std::iter::repeat(Rc::new(RefCell::new(None)))
                .take(variable_cnt)
                .collect(),
            closure_slots: std::iter::repeat(Rc::new(RefCell::new(None)))
                .take(closure_cnt)
                .collect(),
        }
    }
}

#[derive(Debug)]
pub struct Machine {
    program: Rc<Program>,
}

#[derive(Debug)]
struct Environment {
    forks: Vec<(State, OnFork)>,
}

#[derive(Debug, Clone)]
struct State {
    pc: Address,
    stack: PStack<Value>,

    scopes: PVector<(Option<Scope>, PStack<Scope>)>,
    scope_stack: PStack<(Address, ScopeId)>, // (pc, pushed_scope)
    closure_stack: PStack<Closure>,

    iterators: PStack<PathValueIterator>,
}

#[derive(Debug, Clone)]
enum OnFork {
    Nop,
    IgnoreError,
    CatchError,
    SkipCatch,
    Iterate,
}

impl Environment {
    fn new(state: State) -> Self {
        Self {
            forks: vec![(state, OnFork::Nop)],
        }
    }

    fn push_fork(&mut self, state: &State, on_fork: OnFork, new_pc: Address) {
        let mut new_state = state.clone();
        new_state.pc = new_pc;
        self.forks.push((new_state, on_fork));
    }

    fn pop_fork(&mut self) -> Option<(State, OnFork)> {
        self.forks.pop()
    }
}

impl State {
    fn new(pc: Address) -> Self {
        State {
            pc,
            stack: Default::default(),
            scopes: Default::default(),
            scope_stack: Default::default(),
            closure_stack: Default::default(),
            iterators: Default::default(),
        }
    }

    fn push(&mut self, item: Value) {
        self.stack.push(item)
    }

    fn pop(&mut self) -> Value {
        self.stack.pop().ok_or(ProgramError::PopEmptyStack).unwrap()
    }

    fn dup(&mut self) {
        let value = self.pop();
        self.push(value.clone());
        self.push(value);
    }

    fn swap(&mut self) {
        let v1 = self.pop();
        let v2 = self.pop();
        self.push(v1);
        self.push(v2);
    }

    fn push_closure(&mut self, closure: Closure) {
        self.closure_stack.push(closure)
    }

    fn pop_closure(&mut self) -> Closure {
        self.closure_stack
            .pop()
            .ok_or(ProgramError::PopEmptyStack)
            .unwrap()
    }

    fn push_iterator(&mut self, iter: PathValueIterator) {
        self.iterators.push(iter)
    }

    fn pop_iterator(&mut self) {
        self.iterators
            .pop()
            .ok_or(ProgramError::PopEmptyStack)
            .unwrap();
    }

    fn top_iterator(&mut self) -> Option<&mut PathValueIterator> {
        self.iterators.top_mut()
    }

    fn slot(&mut self, scoped_slot: &ScopedSlot) -> RefMut<Option<Value>> {
        let scope = self
            .scopes
            .get_mut(scoped_slot.0 .0)
            .and_then(|(x, _)| x.as_mut())
            .ok_or(ProgramError::UninitializedScope)
            .unwrap();
        scope
            .slots
            .get_mut(scoped_slot.1)
            .ok_or(ProgramError::UnknownSlot)
            .unwrap()
            .borrow_mut()
    }

    fn closure_slot(&mut self, scoped_slot: &ScopedSlot) -> RefMut<Option<Closure>> {
        let scope = self
            .scopes
            .get_mut(scoped_slot.0 .0)
            .and_then(|(x, _)| x.as_mut())
            .ok_or(ProgramError::UninitializedScope)
            .unwrap();
        scope
            .closure_slots
            .get_mut(scoped_slot.1)
            .ok_or(ProgramError::UnknownSlot)
            .unwrap()
            .borrow_mut()
    }

    fn push_scope(
        &mut self,
        scope_id: ScopeId,
        variable_cnt: usize,
        closure_cnt: usize,
        pc: Address,
    ) {
        if self.scopes.len() <= scope_id.0 {
            self.scopes.extend(
                std::iter::repeat_with(|| (None, PStack::new()))
                    .take(scope_id.0 - self.scopes.len() + 1),
            )
        }
        let (scope, stack) = &mut self.scopes[scope_id.0];
        if let Some(prev_scope) = scope.replace(Scope::new(variable_cnt, closure_cnt)) {
            stack.push(prev_scope);
        }
        self.scope_stack.push((pc, scope_id));
    }

    fn pop_scope(&mut self) -> Address {
        let (pc, scope_id) = self
            .scope_stack
            .pop()
            .ok_or(ProgramError::PopEmptyScope)
            .unwrap();
        let (current, prev) = self
            .scopes
            .get_mut(scope_id.0)
            .ok_or(ProgramError::PopUnknownScope)
            .unwrap();
        assert!(current.is_some(), "Pop unknown scope");
        *current = prev.pop();
        pc
    }
}

impl Machine {
    pub fn new(program: Program) -> Self {
        Self {
            program: Rc::new(program),
        }
    }

    pub fn run(&mut self, value: Value) -> ResultIterator {
        let mut state = State::new(self.program.entry_point);
        state.push(value);
        let env = Environment::new(state);
        ResultIterator {
            program: self.program.clone(),
            env,
        }
    }
}

pub struct ResultIterator {
    program: Rc<Program>,
    env: Environment,
}

impl Iterator for ResultIterator {
    type Item = Result<Value>;

    fn next(&mut self) -> Option<Self::Item> {
        run_code(&self.program, &mut self.env)
    }
}

fn run_code(program: &Program, env: &mut Environment) -> Option<Result<Value>> {
    let mut err: Option<QueryExecutionError> = None;
    let mut call_pc: Option<Address> = None;
    let mut catch_skip: usize = 0;
    log::trace!("Start from environment {:?}", env);
    'backtrack: loop {
        let (mut state, on_fork) = if let Some(x) = env.pop_fork() {
            x
        } else {
            return err.map(Err);
        };
        log::trace!("On fork {:?} with state {:?}", on_fork, state);
        match on_fork {
            OnFork::Nop => {
                if err.is_some() {
                    continue 'backtrack;
                }
            }
            OnFork::IgnoreError => {
                if catch_skip == 0 {
                    err = None;
                } else {
                    catch_skip -= 1;
                }
                continue 'backtrack;
            }
            OnFork::CatchError => {
                if catch_skip == 0 {
                    match err.take() {
                        None => continue 'backtrack,
                        Some(e) => state.push(Value::string(format!("{:?}", e))),
                    }
                } else {
                    catch_skip -= 1;
                    continue 'backtrack;
                }
            }
            OnFork::SkipCatch => {
                catch_skip += 1;
                continue 'backtrack;
            }
            OnFork::Iterate => {
                if err.is_some() {
                    continue 'backtrack;
                }
                let it = state.top_iterator().expect("No iterator to iterate on");
                match it.next() {
                    None => {
                        state.pop_iterator();
                        continue 'backtrack;
                    }
                    Some(value) => {
                        env.push_fork(&state, OnFork::Iterate, state.pc);
                        state.push(value.1.clone());
                    }
                }
            }
        }
        assert_eq!(catch_skip, 0);
        log::trace!("Start fork with error {:?}", err);
        'cycle: loop {
            if err.is_some() {
                continue 'backtrack;
            }
            let code = program.fetch_code(state.pc)?;
            log::trace!(
                "Execute code {:?} on stack {:?}, slots = {:?}",
                code,
                state.stack,
                state.scopes
            );
            use ByteCode::*;
            match code {
                Unreachable => panic!("Reached to the unreachable"),
                PlaceHolder => panic!("Reached to a place holder"),
                Nop => {}
                Push(v) => {
                    state.push(v.clone());
                }
                Pop => {
                    state.pop();
                }
                Dup => {
                    state.dup();
                }
                Swap => {
                    state.swap();
                }
                Const(v) => {
                    state.pop();
                    state.push(v.clone())
                }
                Load(scoped_slot) => {
                    let value = state
                        .slot(scoped_slot)
                        .as_ref()
                        .ok_or(ProgramError::UninitializedSlot)
                        .unwrap()
                        .clone();
                    state.push(value);
                }
                Store(scoped_slot) => {
                    let value = state.pop();
                    state.slot(scoped_slot).replace(value);
                }
                PushClosure(closure) => {
                    state.push_closure(*closure);
                }
                StoreClosure(slot) => {
                    let closure = state.pop_closure();
                    state.closure_slot(slot).replace(closure);
                }
                Append(scoped_slot) => {
                    let value = state.pop();
                    let mut slot = state.slot(scoped_slot);
                    let slot_item = slot
                        .as_mut()
                        .ok_or(ProgramError::UninitializedSlot)
                        .unwrap();
                    match slot_item {
                        Value::Array(v) => {
                            v.push_back(value);
                        }
                        _ => {
                            panic!("expected a array to append to, but was not an array");
                        }
                    }
                }
                Index => {
                    let index = state.pop();
                    let value = state.pop();
                    match intrinsic::index(value, index) {
                        Ok((value, _path_elem)) => {
                            state.push(value);
                        }
                        Err(e) => {
                            err.replace(e);
                        }
                    }
                }
                Slice { start, end } => {
                    let end = if *end { Some(state.pop()) } else { None };
                    let start = if *start { Some(state.pop()) } else { None };
                    let value = state.pop();
                    match intrinsic::slice(value, start, end) {
                        Ok((value, _path_elem)) => {
                            state.push(value);
                        }
                        Err(e) => {
                            err.replace(e);
                        }
                    }
                }
                Each => {
                    let value = state.pop();
                    let iter = match value {
                        value
                        @
                        (Value::Null
                        | Value::True
                        | Value::False
                        | Value::Number(_)
                        | Value::String(_)) => {
                            err.replace(QueryExecutionError::IterateOnNonIterable(value));
                            continue 'backtrack;
                        }
                        Value::Array(array) => PathValueIterator::Array { array, next: 0 },
                        Value::Object(map) => {
                            let sorted_elements: PVector<_> = map
                                .into_iter()
                                .sorted_by(|(lhs, _), (rhs, _)| Ord::cmp(lhs, rhs))
                                .collect();
                            PathValueIterator::Object {
                                sorted_elements,
                                next: 0,
                            }
                        }
                    };
                    state.push_iterator(iter);
                    env.push_fork(&state, OnFork::Iterate, state.pc.get_next());
                    continue 'backtrack;
                }
                Fork { fork_pc } => {
                    let fork_pc = *fork_pc;
                    env.push_fork(&state, OnFork::Nop, fork_pc);
                }
                ForkTryBegin { catch_pc } => match catch_pc {
                    None => env.push_fork(&state, OnFork::IgnoreError, state.pc.get_next()),
                    Some(pc) => {
                        let new_pc = *pc;
                        env.push_fork(&state, OnFork::CatchError, new_pc)
                    }
                },
                ForkTryEnd => env.push_fork(&state, OnFork::SkipCatch, state.pc.get_next()),
                Backtrack => continue 'backtrack,
                Jump(address) => {
                    state.pc = *address;
                    continue 'cycle;
                }
                JumpUnless(address) => {
                    let value = state.pop();
                    if !intrinsic::truthy(value) {
                        state.pc = *address;
                        continue 'cycle;
                    }
                }
                CallClosure {
                    slot,
                    return_address,
                } => {
                    let closure = state
                        .closure_slot(slot)
                        .ok_or(ProgramError::UninitializedSlot)
                        .unwrap();
                    assert_eq!(call_pc.replace(*return_address), None);
                    state.pc = closure.0;
                    continue 'cycle;
                }
                Call {
                    function,
                    return_address,
                } => {
                    assert_eq!(call_pc.replace(*return_address), None);
                    state.pc = *function;
                    continue 'cycle;
                }
                NewScope {
                    id,
                    variable_cnt,
                    closure_cnt,
                } => {
                    let return_address = call_pc
                        .take()
                        .expect("NewScope should be called after Call");
                    state.push_scope(*id, *variable_cnt, *closure_cnt, return_address);
                }
                Ret => {
                    let return_address = state.pop_scope();
                    state.pc = return_address;
                    continue 'cycle;
                }
                Output => {
                    let value = state.pop();
                    return Some(Ok(value));
                }
                Intrinsic1(NamedFunction { name: _name, func }) => {
                    let arg = state.pop();
                    match func(arg) {
                        Ok(value) => state.push(value),
                        Err(e) => err = Some(e),
                    }
                }
                Intrinsic2(NamedFunction { name: _name, func }) => {
                    let lhs = state.pop();
                    let rhs = state.pop();
                    match func(lhs, rhs) {
                        Ok(value) => state.push(value),
                        Err(e) => err = Some(e),
                    }
                }
            }
            state.pc.next();
        }
    }
}