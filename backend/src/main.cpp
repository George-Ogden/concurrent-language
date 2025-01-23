#include "main/main.hpp"
#include "system/work_manager.hpp"

#include <iostream>

int main() {
    Main main{};
    WorkManager::run(&main);
    std::cout << main.value() << std::endl;
    return 0;
}
