<?php

declare(strict_types=1);

namespace Expanse\Driver;

class NativeDriver
{
    public static function isAvailable(): bool
    {
        return extension_loaded('expanse') || extension_loaded('expanse-php') || class_exists(\Expanse\ExpanseSet::class, false);
    }
}
