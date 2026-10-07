/* Independent Apple ps RSS API probe; no signals, changes or privileged calls. */
#include <mach/mach.h>
#include <sys/sysctl.h>
#include <sys/user.h>
#include <stdio.h>
#include <stdlib.h>
extern kern_return_t task_read_for_pid(task_port_t, pid_t, task_port_t *);
int main(void) {
 int mib[4]={CTL_KERN,KERN_PROC,KERN_PROC_ALL,0}; size_t size=0;
 if(sysctl(mib,4,NULL,&size,NULL,0)) return 1;
 size+=size/2; struct kinfo_proc *rows=malloc(size);
 if(!rows || sysctl(mib,4,rows,&size,NULL,0)) return 2;
 puts("pid,status,rss_kib");
 for(size_t i=0;i<size/sizeof(*rows);i++) {
  pid_t pid=rows[i].kp_proc.p_pid; if(!pid) continue;
  task_port_t task=MACH_PORT_NULL;
  kern_return_t status=task_read_for_pid(mach_task_self(),pid,&task);
  task_basic_info_data_t info={0}; mach_msg_type_number_t count=TASK_BASIC_INFO_COUNT;
  if(status==KERN_SUCCESS) {
   status=task_info(task,TASK_BASIC_INFO,(task_info_t)&info,&count);
   mach_port_deallocate(mach_task_self(),task);
  }
  printf("%d,%d,%llu\n",pid,status,(unsigned long long)info.resident_size/1024);
 }
 free(rows); return 0;
}
