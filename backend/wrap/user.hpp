#pragma once

#include <pthread.h>
#include <sched.h>
#include <system/thread_manager.hpp>

extern "C" {
int __real_pthread_setaffinity_np(pthread_t thread, size_t cpusetsize,
                                  const cpu_set_t *cpuset);
int __wrap_pthread_setaffinity_np(pthread_t thread, size_t cpusetsize,
                                  const cpu_set_t *cpuset) {
    if (ThreadManager::available_concurrency() <=
        ThreadManager::hardware_concurrency()) {
        // Set affinity if possible.
        return __real_pthread_setaffinity_np(thread, cpusetsize, cpuset);
    }
    return 0;
}
long long int __wrap_pthread_setschedparam(pthread_t __target_thread,
                                           int __policy,
                                           const struct sched_param *__param) {
    // Do not change the priority.
    return 0;
}
}
