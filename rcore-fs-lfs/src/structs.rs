//! On-disk structures in SFS

use crate::vfs;
use alloc::{
    str,
    collections::BTreeMap,
};

use core::fmt::{Debug, Error, Formatter};
use core::mem::{size_of, size_of_val};
use core::slice;
use static_assertions::const_assert;
use spin::RwLock;
use rcore_fs::dirty::Dirty;

/// On-disk superblock
#[repr(C)]
#[derive(Debug)]
pub struct SuperBlock {
    /// magic number, should be LFS_MAGIC
    pub magic: u32,
    /// number of blocks in fs
    pub blocks: u32,
    /// number of unused blocks in fs
    pub unused_blocks: u32,
    /// information for sfs
    pub info: Str32,
    pub current_seg_id: u32,
    pub next_ino_number: u32,
    pub n_segment: u32,
}

/// inode (on disk)
#[repr(C)]
#[derive(Debug)]
pub struct DiskINode {
    /// size of the file (in bytes)
    /// undefined in dir (256 * #entries ?)
    pub size: u32,
    /// one of SYS_TYPE_* above
    pub type_: FileType,
    /// number of hard links to this file
    /// Note: "." and ".." is counted in this nlinks
    pub nlinks: u16,
    /// number of blocks
    pub blocks: u32,
    /// direct blocks
    pub direct: [u32; NDIRECT],
    /// indirect blocks
    pub indirect: u32,
    /// double indirect blocks
    pub db_indirect: u32,
    /// device inode id for char/block device (major, minor)
    pub device_inode_id: usize,
}

/*
pub trait DeviceINode : Any + Sync + Send{
    fn read_at(&self, _offset: usize, buf: &mut [u8]) -> vfs::Result<usize>;
    fn write_at(&self, _offset: usize, buf: &[u8]) -> vfs::Result<usize>;
}
*/
pub type IMapTable = BTreeMap<INodeId, BlockId>;

pub type DeviceINode = dyn vfs::INode;

#[repr(C)]
pub struct IndirectBlock {
    pub entries: [u32; BLK_NENTRY],
}

/// file entry (on disk)
#[repr(C)]
#[derive(Debug)]
pub struct DiskEntry {
    /// inode number
    pub id: u32,
    /// file name
    pub name: Str256,
}
pub struct CheckRegion {
    // pub imaps_blkid: u32,
    pub inodes_num: u32,
}

#[repr(C)]
pub struct Str256(pub [u8; 256]);

#[repr(C)]
pub struct Str32(pub [u8; 32]);

impl AsRef<str> for Str256 {
    fn as_ref(&self) -> &str {
        let len = self.0.iter().enumerate().find(|(_, &b)| b == 0).unwrap().0;
        str::from_utf8(&self.0[0..len]).unwrap()
    }
}

impl AsRef<str> for Str32 {
    fn as_ref(&self) -> &str {
        let len = self.0.iter().enumerate().find(|(_, &b)| b == 0).unwrap().0;
        str::from_utf8(&self.0[0..len]).unwrap()
    }
}

impl Debug for Str256 {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(f, "{}", self.as_ref())
    }
}

impl Debug for Str32 {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(f, "{}", self.as_ref())
    }
}

impl<'a> From<&'a str> for Str256 {
    fn from(s: &'a str) -> Self {
        let mut ret = [0u8; 256];
        ret[0..s.len()].copy_from_slice(s.as_ref());
        Str256(ret)
    }
}

impl<'a> From<&'a str> for Str32 {
    fn from(s: &'a str) -> Self {
        let mut ret = [0u8; 32];
        ret[0..s.len()].copy_from_slice(s.as_ref());
        Str32(ret)
    }
}

impl SuperBlock {
    pub fn check(&self) -> bool {
        self.magic == MAGIC
    }
}

impl DiskINode {
    pub const fn new_file() -> Self {
        DiskINode {
            size: 0,
            type_: FileType::File,
            nlinks: 0,
            blocks: 0,
            direct: [0; NDIRECT],
            indirect: 0,
            db_indirect: 0,
            device_inode_id: NODEVICE,
        }
    }
    pub const fn new_symlink() -> Self {
        DiskINode {
            size: 0,
            type_: FileType::SymLink,
            nlinks: 0,
            blocks: 0,
            direct: [0; NDIRECT],
            indirect: 0,
            db_indirect: 0,
            device_inode_id: NODEVICE,
        }
    }
    pub const fn new_dir() -> Self {
        DiskINode {
            size: 0,
            type_: FileType::Dir,
            nlinks: 0,
            blocks: 0,
            direct: [0; NDIRECT],
            indirect: 0,
            db_indirect: 0,
            device_inode_id: NODEVICE,
        }
    }
    pub const fn new_chardevice(device_inode_id: usize) -> Self {
        DiskINode {
            size: 0,
            type_: FileType::CharDevice,
            nlinks: 0,
            blocks: 0,
            direct: [0; NDIRECT],
            indirect: 0,
            db_indirect: 0,
            device_inode_id: device_inode_id,
        }
    }
}

/// Convert structs to [u8] slice
pub trait AsBuf {
    fn as_buf(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self as *const _ as *const u8, size_of_val(self)) }
    }
    fn as_buf_mut(&mut self) -> &mut [u8] {
        unsafe { slice::from_raw_parts_mut(self as *mut _ as *mut u8, size_of_val(self)) }
    }
}

pub struct SummaryEntry {
    pub inode_id: i32,
    pub entry_id: i32,
}

pub struct SegmentMeta {
    pub size: u32,
    pub inodes_num: u32,
    pub unused: u32,
}

pub struct Segment {
    /// on-disk segment
    pub meta: Dirty<SegmentMeta>,
    pub seg_imap: RwLock<Dirty<IMapTable>>,
    pub summary_map: RwLock<Dirty<BTreeMap<BlockId, SummaryEntry>>>,
    // imap: RwLock<BTreeMap<INodeId, INodeImpl>>,
}

impl AsBuf for SuperBlock {}

impl AsBuf for DiskINode {}

impl AsBuf for DiskEntry {}

impl AsBuf for u32 {}

impl AsBuf for CheckRegion {}

impl AsBuf for SummaryEntry {}

impl AsBuf for SegmentMeta {}

/*
 * Simple FS (SFS) definitions visible to ucore. This covers the on-disk format
 * and is used by tools that work on SFS volumes, such as mksfs.
 */
pub type BlockId = usize;
pub type SegmentId = usize;
pub type INodeId = BlockId;

pub const NODEVICE: usize = 100;

/// magic number for lfs
pub const MAGIC: u32 = 0x2f8dbe2c;
/// size of block
pub const BLKSIZE: usize = 1usize << BLKSIZE_LOG2; // 4KB
/// log2( size of block )
pub const BLKSIZE_LOG2: u8 = 12;
/// number of direct blocks in inode
pub const NDIRECT: usize = 12;
/// default lfs infomation string
pub const DEFAULT_INFO: &str = "log file system";
/// max length of infomation
pub const MAX_INFO_LEN: usize = 31;
/// max length of filename
pub const MAX_FNAME_LEN: usize = 255;
/// max file size in theory (48KB + 4MB + 4GB)
/// however, the file size is stored in u32
pub const MAX_FILE_SIZE: usize = 0xffffffff;
/// block the superblock lives in
pub const BLKN_SUPER: BlockId = 0;
pub const BLKN_CR: BlockId = 1;
pub const BLKN_SEGMENT: BlockId = 0x100;
/// location of the root dir inode
// pub const BLKN_ROOT: BlockId = 1;
pub const INO_ROOT: INodeId = 0;
/// number of bits in a block
pub const BLKBITS: usize = BLKSIZE * 8;
/// size of one entry
pub const ENTRY_SIZE: usize = 4;
/// number of entries in a block
pub const BLK_NENTRY: usize = BLKSIZE / ENTRY_SIZE; // 1024
/// size of a dirent used in the size field
pub const DIRENT_SIZE: usize = MAX_FNAME_LEN + 1 + ENTRY_SIZE; // 270
/// max number of blocks with direct blocks
pub const MAX_NBLOCK_DIRECT: usize = NDIRECT;
/// max number of blocks with indirect blocks
pub const MAX_NBLOCK_INDIRECT: usize = NDIRECT + BLK_NENTRY;
/// max number of blocks with double indirect blocks
pub const MAX_NBLOCK_DOUBLE_INDIRECT: usize = NDIRECT + BLK_NENTRY + BLK_NENTRY * BLK_NENTRY;

/// size of one segment
pub const SEGMENT_BLKS: usize = 1024;
pub const SEGMENT_SIZE: usize = SEGMENT_BLKS * BLKSIZE;

pub const IMAP_PER_SEGMENT_SIZE: usize = BLKSIZE * 2;
pub const SS_PER_SEGMENT_SIZE: usize = BLKSIZE * 2;
pub const SEGMENT_META_SIZE: usize = BLKSIZE;
pub const BLK_DATA_BEGIN: usize = (IMAP_PER_SEGMENT_SIZE + SS_PER_SEGMENT_SIZE + SEGMENT_META_SIZE) / BLKSIZE;
pub const SEGN_ROOT: usize = 1;
pub const ENTRY_SPECIALBLOCK: isize = -1; // for inode, "indirect block"
pub const ENTRY_GARBAGE: isize = -2; // for deleted block
pub const INVALID_INO: isize = -1;
pub const INVALID_BLKID: usize = 0;

/// file types
#[repr(u16)]
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum FileType {
    Invalid = 0,
    File = 1,
    Dir = 2,
    SymLink = 3,
    CharDevice = 4,
    BlockDevice = 5,
}

const_assert!(o1; size_of::<SuperBlock>() <= BLKSIZE);
const_assert!(o2; size_of::<DiskINode>() <= BLKSIZE);
const_assert!(o3; size_of::<DiskEntry>() <= BLKSIZE);
const_assert!(o4; size_of::<IndirectBlock>() == BLKSIZE);
const_assert!(o5; DEFAULT_INFO.len() <= MAX_INFO_LEN);
