#pragma once

#include <algorithm>
#include <cstddef>
#include <unistd.h>
#include <vector>

class cache_utils {
    static inline unsigned level = 1;

  public:
    static size_t get_line_size(unsigned level = get_default_level()) {
        long long line_size = -1;
        switch (level) {
        case 1:
            line_size = sysconf(_SC_LEVEL1_DCACHE_LINESIZE);
            break;
        case 2:
            line_size = sysconf(_SC_LEVEL2_CACHE_LINESIZE);
            break;
        case 3:
            line_size = sysconf(_SC_LEVEL3_CACHE_LINESIZE);
            break;
        case 4:
            line_size = sysconf(_SC_LEVEL4_CACHE_LINESIZE);
            break;
        }
        return std::max(line_size, 0LL);
    }

    static size_t get_cache_size(unsigned level = get_default_level()) {
        long long cache_size = -1;
        switch (level) {
        case 1:
            cache_size = sysconf(_SC_LEVEL1_DCACHE_SIZE);
            break;
        case 2:
            cache_size = sysconf(_SC_LEVEL2_CACHE_SIZE);
            break;
        case 3:
            cache_size = sysconf(_SC_LEVEL3_CACHE_SIZE);
            break;
        case 4:
            cache_size = sysconf(_SC_LEVEL4_CACHE_SIZE);
            break;
        }
        return std::max(cache_size, 0LL);
    }

    static unsigned get_num_lines(unsigned level = get_default_level()) {
        size_t cache_size = cache_utils::get_cache_size(level);
        size_t line_size = cache_utils::get_line_size(level);
        if (cache_size == 0 || line_size == 0) {
            return 0;
        }
        return cache_size / line_size;
    }

    static unsigned get_associativity(unsigned level = get_default_level()) {
        long long associativity = -1;
        switch (level) {
        case 1:
            associativity = sysconf(_SC_LEVEL1_DCACHE_ASSOC);
            break;
        case 2:
            associativity = sysconf(_SC_LEVEL2_CACHE_ASSOC);
            break;
        case 3:
            associativity = sysconf(_SC_LEVEL3_CACHE_ASSOC);
            break;
        case 4:
            associativity = sysconf(_SC_LEVEL4_CACHE_ASSOC);
            break;
        }
        return std::max(associativity, 0LL);
    }

    static unsigned get_default_level() { return cache_utils::level; }
    static void set_default_level(unsigned level) {
        cache_utils::level = level;
    }
};
