use nix::ioctl_readwrite;

const MAGIC: u8 = b'I';

ioctl_readwrite!(ion_alloc, MAGIC, 0, IonAllocationData);
ioctl_readwrite!(ion_free, MAGIC, 1, IonHandleData);
ioctl_readwrite!(ion_map, MAGIC, 2, IonFdData);

pub const ION_HEAP_MASK_CARVEOUT: libc::c_uint = 4;

type IonUserHandle = libc::c_int;

#[repr(C)]
pub struct IonAllocationData {
    pub len: libc::size_t,
    pub align: libc::size_t,
    pub heap_id_mask: libc::c_uint,
    pub flags: libc::c_uint,
    pub handle: IonUserHandle,
}

#[repr(C)]
pub struct IonHandleData {
    pub handle: IonUserHandle,
}

#[repr(C)]
pub struct IonFdData {
    pub handle: IonUserHandle,
    pub fd: libc::c_int,
}
