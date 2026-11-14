// SCSI commands to an optical drive on macOS through the MMC user client (SCSITaskLib).
// The drive must not be in use: macOS grants exclusive access only while no disc is mounted.

#include <string.h>
#include <CoreFoundation/CoreFoundation.h>
#include <IOKit/IOKitLib.h>
#include <IOKit/IOCFPlugIn.h>
#include <IOKit/scsi/SCSITaskLib.h>

// The nth IOCDBlockStorageDevice, in the order the I/O Registry lists them.
static io_service_t nth_drive(unsigned index) {
    io_iterator_t it;
    if (IOServiceGetMatchingServices(MACH_PORT_NULL, IOServiceMatching("IOCDBlockStorageDevice"), &it) != KERN_SUCCESS) {
        return 0;
    }
    io_service_t s = 0;
    for (unsigned i = 0; (s = IOIteratorNext(it)) != 0; i++) {
        if (i == index) {
            break;
        }
        IOObjectRelease(s);
    }
    IOObjectRelease(it);
    return s;
}

// The ancestor that offers the authoring (MMC) user client.
static io_service_t authoring_nub(io_service_t s) {
    IOObjectRetain(s);
    for (int depth = 0; depth < 8 && s; depth++) {
        CFTypeRef cat = IORegistryEntryCreateCFProperty(s, CFSTR("SCSITaskDeviceCategory"), NULL, 0);
        if (cat) {
            CFRelease(cat);
            return s;
        }
        io_service_t parent = 0;
        kern_return_t kr = IORegistryEntryGetParentEntry(s, kIOServicePlane, &parent);
        IOObjectRelease(s);
        s = kr == KERN_SUCCESS ? parent : 0;
    }
    if (s) {
        IOObjectRelease(s);
    }
    return 0;
}

// Runs `cdb` on drive `index` and reads up to `len` bytes into `buf`; `*got` is how many came.
// Returns 0, or a kern_return_t / SCSI status that tells what failed (-1: no drive, -2: no interface).
int psy_scsi_command_in(unsigned index, const unsigned char *cdb, unsigned cdb_len, unsigned char *buf, unsigned len, unsigned *got) {
    *got = 0;
    if (cdb_len > 16) {
        return -2;
    }
    io_service_t drive = nth_drive(index);
    if (!drive) {
        return -1;
    }
    io_service_t nub = authoring_nub(drive);
    IOObjectRelease(drive);
    if (!nub) {
        return -2;
    }
    IOCFPlugInInterface **plug = NULL;
    SInt32 score = 0;
    kern_return_t kr = IOCreatePlugInInterfaceForService(nub, kIOMMCDeviceUserClientTypeID, kIOCFPlugInInterfaceID, &plug, &score);
    IOObjectRelease(nub);
    if (kr != KERN_SUCCESS || !plug) {
        return kr ? kr : -2;
    }
    MMCDeviceInterface **mmc = NULL;
    (*plug)->QueryInterface(plug, CFUUIDGetUUIDBytes(kIOMMCDeviceInterfaceID), (LPVOID *)&mmc);
    int result = -2;
    if (mmc) {
        SCSITaskDeviceInterface **dev = (*mmc)->GetSCSITaskDeviceInterface(mmc);
        if (dev) {
            kr = (*dev)->ObtainExclusiveAccess(dev);
            if (kr != KERN_SUCCESS) {
                result = kr;
            } else {
                SCSITaskInterface **task = (*dev)->CreateSCSITask(dev);
                if (task) {
                    SCSICommandDescriptorBlock block;
                    memset(block, 0, sizeof block);
                    memcpy(block, cdb, cdb_len);
                    (*task)->SetCommandDescriptorBlock(task, block, (UInt8)cdb_len);
                    IOVirtualRange range = {(IOVirtualAddress)buf, len};
                    if (len) {
                        (*task)->SetScatterGatherEntries(task, &range, 1, len, kSCSIDataTransfer_FromTargetToInitiator);
                    } else {
                        (*task)->SetScatterGatherEntries(task, NULL, 0, 0, kSCSIDataTransfer_NoDataTransfer);
                    }
                    (*task)->SetTimeoutDuration(task, 10000);
                    SCSI_Sense_Data sense;
                    SCSITaskStatus status = kSCSITaskStatus_No_Status;
                    UInt64 transferred = 0;
                    kr = (*task)->ExecuteTaskSync(task, &sense, &status, &transferred);
                    result = kr != KERN_SUCCESS ? kr : (status != kSCSITaskStatus_GOOD ? (int)status : 0);
                    *got = (unsigned)transferred;
                    (*task)->Release(task);
                }
                (*dev)->ReleaseExclusiveAccess(dev);
            }
            (*dev)->Release(dev);
        }
        (*mmc)->Release(mmc);
    }
    IODestroyPlugInInterface(plug);
    return result;
}
