#pragma once

#include "work/work.tpp"

struct WorkSizeComparator {
    bool operator()(const WorkT &a, const WorkT &b) const { return *a < *b; }
};
