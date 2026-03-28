import debounce from "@/helpers/debounce";
import { saveToJson } from "@/services/storage/json.electron";

export const debouncedSave = debounce(saveToJson, 400);
