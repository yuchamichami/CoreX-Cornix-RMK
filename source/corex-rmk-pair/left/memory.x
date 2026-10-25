MEMORY
{
  /* Raytac keeps the installed S140 below the selected app base.
     Stock starts at 0x1000. Both preserve 0xA0000 and above;
     RMK settings occupy 0xC0000..0xE0000. */
  FLASH : ORIGIN = DEFINED(COREX_RAYTAC_FLASH_BASE) ? COREX_RAYTAC_FLASH_BASE : 0x1000,
          LENGTH = 0xA0000 - (DEFINED(COREX_RAYTAC_FLASH_BASE) ? COREX_RAYTAC_FLASH_BASE : 0x1000)
  RAM : ORIGIN = 0x20000008, LENGTH = 255K
}
