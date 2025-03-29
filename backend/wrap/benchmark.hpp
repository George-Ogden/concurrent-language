#pragma once

#include <pthread.h>
#include <sched.h>

extern "C" {
long long int __real_pthread_setschedparam(pthread_t __target_thread,
                                           int __policy,
                                           const struct sched_param *__param);
long long int __wrap_pthread_setschedparam(pthread_t target_thread, int policy,
                                           struct sched_param *param) {
    // Allow changing the priority (but decrement it first).
    (*param).sched_priority--;
    return __real_pthread_setschedparam(target_thread, policy, param);
}
int __real_pthread_setaffinity_np(pthread_t thread, size_t cpusetsize,
                                  const cpu_set_t *cpuset);
int __wrap_pthread_setaffinity_np(pthread_t thread, size_t cpusetsize,
                                  const cpu_set_t *cpuset) {
    // Set the affinity.
    return __real_pthread_setaffinity_np(thread, cpusetsize, cpuset);
}
}
