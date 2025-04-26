#![no_std]

use core::{
    marker::PhantomData,
    sync::atomic::{AtomicPtr, Ordering},
};

use crate_interface::{call_interface, def_interface};

/// 用于位置无关指针的接口，需要实现该接口后才可使用位置无关指针
#[def_interface]
pub trait GetDataBase {
    /// 返回当前地址空间下，共享区域的首地址
    fn get_data_base() -> usize;
}

/// 代表空指针
///
/// 不能用0代表空指针，因为位置无关指针以偏移量存储，偏移量可能为0
///
/// 低位尽可能为0，从而与可能的标记兼容
pub const NULL_PTR: usize = 0x8000_0000_8000_0000;

/// 用于实现位置无关指针
///
/// 用户也可以使用该trait描述更复杂的指针类型，例如带标记的指针
///
/// 实现该trait的类型需要具有与`*mut ()`相同的内存布局
///
/// 且对于该类型的对象`p`，需要满足：
///
/// `p.value() = *(&p as *const () as *const *mut ())`
pub trait WrappedPtr {
    /// 获取该对象的值，没有进行去标记和地址转换
    fn value(&self) -> *mut ();
    /// 获取该对象的值，且经过了某种将值变为有效的指针的转换
    ///
    /// 在未嵌套的情况下，返回值可以直接作为指针访问内存地址
    ///
    /// 在嵌套的情况下，最外层类型的`ptr`函数的返回值可以直接作为指针访问内存地址
    fn ptr(&self) -> *mut ();
    /// 新建对象，直接将传入的值存入
    fn from_value(value: *mut ()) -> Self;
    /// 新建对象，认为传入的值是指针，对其做ptr函数内变换的逆变换后存入结构
    fn from_ptr(ptr: *mut ()) -> Self;
    /// 修改该对象存储的值
    ///
    /// 直接将传入的值存入对象
    fn set(&mut self, value: *mut ());
    /// 判断对象中存储的是不是空指针
    ///
    /// 对于每一层抽象，判断标准可能不同
    ///
    /// 例如，对于位置无关指针而言，是将其`value()`与`NULL_PTR`比较
    ///
    /// 对于被标记的指针而言，则要先去掉标记，再执行内部的判断逻辑
    fn is_null(&self) -> bool;

    /// 创建空指针
    ///
    /// 使用`NULL_PTR = 0x8000_0000_8000_0000`代表空指针
    fn null() -> Self
    where
        Self: Sized,
    {
        Self::from_value(NULL_PTR as *mut ())
    }
}

/// 用于vdso的位置无关指针
///
/// 该指针可以用于在被多个地址空间共享的区域内实现自引用
///
/// 指针内存储的值为其目标地址相对共享区域首地址的偏移
///
/// 本类型通过`get_data_base`函数获取共享区域首地址，从而在实际指针和偏移间转换
#[derive(Copy, Clone)]
pub struct PIPtr(*mut ());

impl WrappedPtr for PIPtr {
    /// 获取相对偏移量，也就是该指针变量实际存储的值
    ///
    /// `self.value() = self.0`
    fn value(&self) -> *mut () {
        self.0
    }

    /// 获取可以直接寻址的指针（可寻址的前提是指针非空）
    ///
    /// `self.ptr() = if self.0 == NULL_PTR { NULL_PTR } else { self.0 - get_data_base() }`
    fn ptr(&self) -> *mut () {
        if self.0 as usize == NULL_PTR {
            NULL_PTR as *mut ()
        } else {
            (self.0 as usize + call_interface!(GetDataBase::get_data_base())) as *mut ()
        }
    }

    /// 认为传入的地址为相对偏移量，从而创建位置无关指针
    ///
    /// `self.value = value`
    fn from_value(value: *mut ()) -> Self {
        Self(value)
    }

    /// 认为传入的地址为指针，经过转换后创建位置无关指针
    ///
    /// `self.value = if ptr == NULL_PTR { NULL_PTR } else { ptr  - get_data_base() }`
    fn from_ptr(ptr: *mut ()) -> Self {
        if ptr as usize == NULL_PTR {
            Self(NULL_PTR as *mut ())
        } else {
            Self((ptr as usize - call_interface!(GetDataBase::get_data_base())) as *mut ())
        }
    }

    /// 认为传入的地址为相对偏移量，为该对象赋值
    ///
    /// `self.value = value`
    fn set(&mut self, value: *mut ()) {
        self.0 = value;
    }

    fn is_null(&self) -> bool {
        self.0 as usize == NULL_PTR
    }
}

impl WrappedPtr for *mut () {
    /// 为了方便起见，为Rust的指针类型也实现了`WrappedPtr` trait
    ///
    /// 对一般指针而言，`value`、`ptr`和存储的实际值是相同的
    fn value(&self) -> *mut () {
        *self
    }

    /// 为了方便起见，为Rust的指针类型也实现了`WrappedPtr` trait
    ///
    /// 对一般指针而言，`value`、`ptr`和存储的实际值是相同的
    fn ptr(&self) -> *mut () {
        *self
    }

    /// 为了方便起见，为Rust的指针类型也实现了`WrappedPtr` trait
    ///
    /// 对一般指针而言，`value`、`ptr`和存储的实际值是相同的
    fn from_value(value: *mut ()) -> Self {
        value
    }

    /// 为了方便起见，为Rust的指针类型也实现了`WrappedPtr` trait
    ///
    /// 对一般指针而言，`value`、`ptr`和存储的实际值是相同的
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

/// 由于共享区域内的指针可能有同步需求，因此实现了此`WrappedPtr`的原子版本
pub struct AtomicWrappedPtr<T: WrappedPtr> {
    /// 将指针按`value`存储
    inner: AtomicPtr<()>,
    _phantom: PhantomData<T>,
}

impl<T> AtomicWrappedPtr<T>
where
    T: WrappedPtr,
{
    /// 获取该指针变量实际存储的值
    pub fn load_value(&self) -> *mut () {
        self.inner.load(Ordering::Acquire)
    }

    /// 获取可以直接寻址的指针（可寻址的前提是指针非空）
    pub fn load_ptr(&self) -> *mut () {
        T::from_value(self.inner.load(Ordering::Acquire)).ptr()
    }

    /// 获取其内部数据的拷贝
    pub fn load(&self) -> T {
        T::from_value(self.inner.load(Ordering::Acquire))
    }

    /// 将传入的地址数据直接存储，不经过转换，从而创建对象
    pub fn from_value(value: *mut ()) -> Self {
        Self {
            inner: AtomicPtr::new(value),
            _phantom: PhantomData,
        }
    }

    /// 认为传入的地址为指针，经过转换后存储，从而创建对象
    pub fn from_ptr(ptr: *mut ()) -> Self {
        Self {
            inner: AtomicPtr::new(T::from_ptr(ptr).value()),
            _phantom: PhantomData,
        }
    }

    /// 创建空指针
    ///
    /// 使用`NULL_PTR = 0x8000_0000_8000_0000`代表空指针
    pub const fn null() -> Self {
        Self {
            inner: AtomicPtr::new(NULL_PTR as *mut ()),
            _phantom: PhantomData,
        }
    }

    /// 将传入的地址数据直接赋值给对象，不经过转换
    pub fn store(&self, value: *mut ()) {
        self.inner.store(value, Ordering::Release);
    }

    /// 对该对象进行CAS操作，所有参数和返回值都不经过转换
    pub fn compare_exchange(&self, current: *mut (), new: *mut ()) -> Result<*mut (), *mut ()> {
        self.inner
            .compare_exchange(current, new, Ordering::AcqRel, Ordering::Acquire)
    }
}

/// 位置无关指针的原子版本
pub type AtomicPIPtr = AtomicWrappedPtr<PIPtr>;
