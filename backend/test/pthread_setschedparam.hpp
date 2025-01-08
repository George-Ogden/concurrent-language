#pragma once

#include <pthread.h>

extern "C" {
long long int __wrap_pthread_setschedparam(pthread_t __target_thread,
                                           int __policy,
                                           const struct sched_param *__param) {
    return 0;
}
}
