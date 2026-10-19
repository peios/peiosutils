#include "peios/reg.h"
#include "peios/regman.h"
#include "peios/evman.h"

static_assert(sizeof(peios_reg_bytes) == sizeof(peios_regman_bytes));
static_assert(sizeof(peios_reg_bytes) == sizeof(peios_evman_bytes));
