#pragma once

#include "work/status.hpp"
#include "system/thread_manager.tpp"

#include <atomic>

Status::Status() = default;

bool Status::acquire(){
    return value.compare_exchange<OWNED_IDX>(false, true, std::memory_order_acquire);
}

bool Status::release(){
    return value.compare_exchange<PRIORITY_IDX,OWNED_IDX>(false, false, std::memory_order_acquire);
}

bool Status::prioritize(){
    return value.compare_exchange<PRIORITY_IDX>(false, true);
}

bool Status::priority() const{
    return value.load<PRIORITY_IDX>();
}

void Status::finish(){
    value.store<DONE_IDX>(true);
}

bool Status::done() const{
    return value.load<DONE_IDX>();
}
