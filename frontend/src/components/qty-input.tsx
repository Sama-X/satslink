import { COLORS } from "@/utils/colors";
import { Result } from "@/utils/types";
import { createSignal, For, JSX, onMount } from "solid-js";

interface QtyInputProps {
  value?: number;
  symbol?: string;
  placeholder?: string;
  onChange: (r: Result<number>) => void;
  validations?: { required?: null; min?: number; max?: number; pattern?: string }[];
}

export const QtyInput = (props: QtyInputProps) => {
  const [value, setValue] = createSignal(props.value || 0);
  const [error, setError] = createSignal<string | null>(null);

  const validate = (val: number | string) => {
    if (!props.validations) return null;

    for (const v of props.validations) {
      if (v.required !== undefined && !val) {
        return "Required";
      }

      if (v.min !== undefined && Number(val) < v.min) {
        return `Must be at least ${v.min}`;
      }

      if (v.max !== undefined && Number(val) > v.max) {
        return `Must be no more than ${v.max}`;
      }

      if (v.pattern !== undefined) {
        const regex = new RegExp(v.pattern);
        if (!regex.test(String(val))) {
          return "Invalid format";
        }
      }
    }

    return null;
  };

  const handleChange = (e: JSX.TargetedEvent<HTMLInputElement, Event>) => {
    const val = e.target.value;
    setValue(Number(val));

    const err = validate(val);
    setError(err);

    if (err) {
      props.onChange(Result.Err(Number(val)));
    } else {
      props.onChange(Result.Ok(Number(val)));
    }
  };

  onMount(() => {
    props.onChange(Result.Ok(value()));
  });

  return (
    <div class="flex flex-col gap-2">
      <div class="flex items-center rounded-md border border-gray-700 bg-gray-900 focus-within:border-gray-500">
        <input
          type="number"
          step="0.00000001"
          placeholder={props.placeholder || "0.0"}
          class="flex-grow rounded-md bg-transparent py-2.5 pl-3 text-sm font-medium text-gray-140 outline-none"
          value={value()}
          onChange={handleChange}
        />
        {props.symbol && (
          <span class="mr-3 text-sm font-medium text-gray-400">{props.symbol}</span>
        )}
      </div>
      {error() && (
        <p class="text-xs text-errorRed">
          {error()}
        </p>
      )}
    </div>
  );
};