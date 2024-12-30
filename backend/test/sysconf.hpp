#pragma once

#include <unistd.h>

extern "C" {
long long __wrap_sysconf(int name) {
    switch (name) {
    case (_SC_LEVEL1_DCACHE_LINESIZE):
        return 64;
    case (_SC_LEVEL2_CACHE_LINESIZE):
        return 256;
    case (_SC_LEVEL3_CACHE_LINESIZE):
        return 256;
    case (_SC_LEVEL1_DCACHE_SIZE):
        return 49152;
    case (_SC_LEVEL2_CACHE_SIZE):
        return 1310720;
    case (_SC_LEVEL3_CACHE_SIZE):
        return 4194304;
    case (_SC_LEVEL4_CACHE_SIZE):
        return 0;
    case (_SC_LEVEL1_DCACHE_ASSOC):
        return 12;
    case (_SC_LEVEL2_CACHE_ASSOC):
        return 16;
    case (_SC_LEVEL3_CACHE_ASSOC):
        return 8;
    }

    return -1;
}
}
