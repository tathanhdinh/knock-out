#include <linux/fs.h>
#include <linux/cdev.h>
#include <linux/kernel.h>
#include <linux/module.h>
#include <linux/uaccess.h>
#include <linux/slab.h>

MODULE_AUTHOR("Sascha Grunert <mail@saschagrunert.de>");
MODULE_DESCRIPTION("A simple kernel module");
MODULE_LICENSE("MIT");
MODULE_VERSION("0.1.0");

struct module *owner = THIS_MODULE;

struct cdev cdev_buffer;
unsigned char *cdev_ptr = (unsigned char *)&cdev_buffer;

struct file_operations fops;
unsigned char *fops_ptr = (unsigned char *)&fops;

void **parrot_owner_ptr = (void **)&fops.owner;
void **parrot_open_ptr = (void **)&fops.open;
void **parrot_read_ptr = (void **)&fops.read;
void **parrot_release_ptr = (void **)&fops.release;

extern unsigned long copy_to_user_ffi(void *to, const void *from, unsigned long count) {
    return copy_to_user(to, from, count);
};

extern int init_module(void);
extern void cleanup_module(void);

/* MODULE_LICENSE("GPL v2"); */
