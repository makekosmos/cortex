import debounce from '@/helpers/debounce';
import { saveToJson } from '@/services/storage/json.tauri';

export const debouncedSave = debounce(saveToJson, 400);
