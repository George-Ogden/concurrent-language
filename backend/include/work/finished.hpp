#pragma once

#include "work/work.hpp"

/// Used to signify that all work is done.
struct FinishedWork : Work {
    void run() override;
    void await_all() override;
    bool can_respond() const override;
    bool execute_immediately() const override;
};
