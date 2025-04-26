#pragma once

#include "work/work_request.hpp"
#include "work/work.tpp"

WorkRequest::WorkRequest() = default;

void WorkRequest::request() {
    status.request();
}


bool WorkRequest::enqueue() {
    return status.enqueue();
}

void WorkRequest::fulfill() {
    WorkT work;
    do {
        work = this->work.load(std::memory_order_relaxed);
    } while (work == nullptr);
    work->run();
    this->work.store(nullptr, std::memory_order_relaxed);
    status.complete();
}

bool WorkRequest::full() const {
    return status.full();
}

bool WorkRequest::cancel() {
    return status.cancel();
}

bool WorkRequest::fill(const WorkT &work) {
    if (status.fill()){
        this->work.store(work, std::memory_order_relaxed);
        return true;
    } else {
        return false;
    }
}
