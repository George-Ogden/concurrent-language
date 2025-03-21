#pragma once

#include "work/status.hpp"
#include "work/work.hpp"

#include <atomic>

struct WorkRequest {
    Status status;
    std::atomic<WorkT> work;

    WorkRequest();
    void request();
    bool enqueue();
    void fulfill();
    bool full() const;
    bool fill(WorkT &work);
    bool cancel();
};
