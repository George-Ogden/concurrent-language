#pragma once

#include "work/status.hpp"
#include "system/thread_manager.tpp"

#include <atomic>

Status::Status() = default;

bool Status::acquire(){
    auto id = ThreadManager::get_id();;
    return value.compare_exchange<OWNER_IDX>(0, id + 1, std::memory_order_acquire);
}

bool Status::release(){
    return value.compare_exchange<REQUIRED_IDX,OWNER_IDX>(0, 0, std::memory_order_acquire);
}

bool Status::require(){
    return value.compare_exchange<REQUIRED_IDX>(false, true);
}

bool Status::required() const{
    return value.load<REQUIRED_IDX>();
}

void Status::finish(){
    value.store<DONE_IDX>(true);
}

bool Status::done() const{
    return value.load<DONE_IDX>();
}
