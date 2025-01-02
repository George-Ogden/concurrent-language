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
        return __real_pthread_setaffinity_np(thread, cpusetsize, cpuset);
    }
    return 0;
}
}
