#![no_std]

use core::{
    marker::PhantomData,
    sync::atomic::{AtomicPtr, Ordering},
};

use crate_interface::{call_interface, def_interface};

#[def_interface]
pub trait GetDataBase {
    fn get_data_base() -> usize;
}

/// 代表空指针
/// 不能用0代表空指针，因为位置无关指针以偏移量存储，偏移量可能为0
/// 低位尽可能为0，从而与可能的标记兼容
pub static NULL_PTR: usize = 0x8000_0000_8000_0000;

/// 用于实现位置无关指针
/// 用户也可以使用该trait描述更复杂的指针类型，例如带标记的指针
/// 实现该trait的类型需要实现与*mut ()的可逆转换：
/// P: WrappedPtr; from_value(P.value) == P
/// p: *mut (); from_value(p).value == p
pub trait WrappedPtr {
    /// 获取该对象的值，没有进行去标记和地址转换
    fn value(&self) -> *mut ();
    /// 获取该对象的值，且经过了某种将值变为有效的指针的转换。
    /// 在未嵌套的情况下，返回值可以直接作为指针访问内存地址。
    /// 在嵌套的情况下，最外层类型的ptr函数的返回值可以直接作为指针访问内存地址。
    fn ptr(&self) -> *mut ();
    /// 新建对象，直接将传入的值存入。
    fn from_value(value: *mut ()) -> Self;
    /// 新建对象，认为传入的值是指针，对其做ptr函数内变换的逆变换后存入结构。
    fn from_ptr(ptr: *mut ()) -> Self;
    /// 修改该对象存储的值。
    /// 直接将传入的值存入对象。
    fn set(&mut self, value: *mut ());
    /// 判断对象中存储的是不是空指针
    /// 对于每一层抽象，判断标准可能不同。
    /// 例如，对于位置无关指针而言，是将其value与NULL_PTR比较。
    /// 对于被标记的指针而言，则要先去掉标记，再执行内部的判断逻辑。
    fn is_null(&self) -> bool;

    /// 创建空指针
    /// 使用NULL_PTR = 0x8000_0000_8000_0000代表空指针
    fn null() -> Self
    where
        Self: Sized,
    {
        Self::from_value(NULL_PTR as *mut ())
    }
}

#[derive(Copy, Clone)]
pub struct PIPtr(*mut ());

impl WrappedPtr for PIPtr {
    fn value(&self) -> *mut () {
        self.0
    }

    fn ptr(&self) -> *mut () {
        if self.0 as usize == NULL_PTR {
            NULL_PTR as *mut ()
        } else {
            (self.0 as usize + call_interface!(GetDataBase::get_data_base())) as *mut ()
        }
    }

    fn from_value(value: *mut ()) -> Self {
        Self(value)
    }

    fn from_ptr(ptr: *mut ()) -> Self {
        if ptr as usize == NULL_PTR {
            Self(NULL_PTR as *mut ())
        } else {
            Self((ptr as usize - call_interface!(GetDataBase::get_data_base())) as *mut ())
        }
    }

    fn set(&mut self, value: *mut ()) {
        self.0 = value;
    }

    fn is_null(&self) -> bool {
        self.0 as usize == NULL_PTR
    }
}

impl WrappedPtr for *mut () {
    fn value(&self) -> *mut () {
        *self
    }

    fn ptr(&self) -> *mut () {
        *self
    }

    fn from_value(value: *mut ()) -> Self {
        value
    }

    fn from_ptr(ptr: *mut ()) -> Self {
        ptr
    }

    fn set(&mut self, value: *mut ()) {
        *self = value
    }

    fn is_null(&self) -> bool {
        *self as usize == NULL_PTR
    }
}

pub struct AtomicWrappedPtr<T: WrappedPtr> {
    /// 将指针按value存储
    inner: AtomicPtr<()>,
    _phantom: PhantomData<T>,
}

impl<T> AtomicWrappedPtr<T>
where
    T: WrappedPtr,
{
    pub fn load_value(&self) -> *mut () {
        self.inner.load(Ordering::Acquire)
    }

    pub fn load_ptr(&self) -> *mut () {
        T::from_value(self.inner.load(Ordering::Acquire)).ptr()
    }

    pub fn load(&self) -> T {
        T::from_value(self.inner.load(Ordering::Acquire))
    }

    pub fn from_value(value: *mut ()) -> Self {
        Self {
            inner: AtomicPtr::new(value),
            _phantom: PhantomData,
        }
    }

    pub fn from_ptr(ptr: *mut ()) -> Self {
        Self {
            inner: AtomicPtr::new(T::from_ptr(ptr).value()),
            _phantom: PhantomData,
        }
    }

    pub const fn null() -> Self {
        Self {
            inner: AtomicPtr::new(NULL_PTR as *mut ()),
            _phantom: PhantomData,
        }
    }

    pub fn store(&self, value: *mut ()) {
        self.inner.store(value, Ordering::Release);
    }

    pub fn compare_exchange(&self, current: *mut (), new: *mut ()) -> Result<*mut (), *mut ()> {
        self.inner
            .compare_exchange(current, new, Ordering::AcqRel, Ordering::Acquire)
    }
}
