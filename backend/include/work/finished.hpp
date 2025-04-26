#pragma once

#include "work/work.hpp"

struct FinishedWork : Work {
    void run() override;
    void await_all() override;
    bool can_respond() const override;
};
