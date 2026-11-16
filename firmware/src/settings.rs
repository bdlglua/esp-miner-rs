//! Only the dedicated axe_cfg partition is writable. Old slot survives a torn save.
use axeos_core::{
    storage::{self, SLOT_SIZE},
    Config,
};
use esp_storage::FlashStorage;

pub struct SettingsStore {
    flash: FlashStorage<'static>,
    offset: Option<u32>,
    slot: usize,
    generation: u32,
}
impl SettingsStore {
    pub fn load(mut flash: FlashStorage<'static>, defaults: Config) -> (Self, Config) {
        let mut table = [0u8; 0xc00];
        let offset = if flash.read(0x8000, &mut table).is_ok() {
            storage::find_partition(&table, flash.capacity() as u32)
        } else {
            None
        };
        let mut selected = None;
        if let Some(offset) = offset {
            let mut buf = [0; SLOT_SIZE];
            for slot in 0..2 {
                if flash
                    .read(offset + slot as u32 * SLOT_SIZE as u32, &mut buf)
                    .is_ok()
                {
                    if let Some((generation, config)) = storage::decode(&buf) {
                        if config
                            .validate(crate::board::VOLTAGE_MIN_MV, crate::board::VOLTAGE_MAX_MV)
                            .is_ok()
                            && selected.as_ref().is_none_or(|(_, previous, _)| {
                                storage::newer(generation, *previous)
                            })
                        {
                            selected = Some((slot, generation, config));
                        }
                    }
                }
            }
        }
        let (slot, generation, config) = selected.unwrap_or((1, 0, defaults));
        (
            Self {
                flash,
                offset,
                slot,
                generation,
            },
            config,
        )
    }
    pub fn available(&self) -> bool {
        self.offset.is_some()
    }
    pub fn save(&mut self, config: &Config) -> Result<(), &'static str> {
        let offset = self
            .offset
            .ok_or("Partition axe_cfg absente : flasher la table de partitions fournie.")?;
        let mut buf = [0; SLOT_SIZE];
        let generation = self.generation.wrapping_add(1);
        let size = storage::encode(config, generation, &mut buf)?;
        let slot = 1 - self.slot;
        let address = offset + slot as u32 * SLOT_SIZE as u32;
        self.flash
            .erase(address, address + SLOT_SIZE as u32)
            .map_err(|_| "Échec d’effacement de la configuration.")?;
        self.flash
            .write_nor(address + 16, &buf[16..size])
            .map_err(|_| "Échec d’écriture de la configuration.")?;
        self.flash
            .write_nor(address, &buf[..16])
            .map_err(|_| "Échec de validation de la configuration.")?;
        self.flash
            .read(address, &mut buf)
            .map_err(|_| "Échec de relecture flash.")?;
        if storage::decode(&buf) != Some((generation, config.clone())) {
            return Err("Configuration non vérifiée.");
        }
        self.generation = generation;
        self.slot = slot;
        Ok(())
    }
}

static STORE: embassy_sync::blocking_mutex::Mutex<
    embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
    core::cell::RefCell<Option<SettingsStore>>,
> = embassy_sync::blocking_mutex::Mutex::new(core::cell::RefCell::new(None));
pub fn initialize(store: SettingsStore) {
    STORE.lock(|s| *s.borrow_mut() = Some(store));
}
pub fn save(config: &Config) -> Result<(), &'static str> {
    STORE.lock(|s| {
        s.borrow_mut()
            .as_mut()
            .ok_or("Stockage indisponible")?
            .save(config)
    })
}
