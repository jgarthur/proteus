import { useEffect, useRef, useState } from 'react';
import { assemble, disassemble, type AsmError } from '../../lib/opcodes';
import styles from './ConfigEditor.module.css';

interface AssemblyFieldProps {
  code: number[];
  disabled: boolean;
  title?: string;
  onChange(code: number[]): void;
}

/**
 * Mnemonic view of a seed program's bytes.
 *
 * Typing assembles live: clean text is pushed straight to `onChange`, and text
 * that does not assemble is kept in the box with its errors listed underneath
 * while the last valid bytes stay in the config. The raw-bytes input remains
 * the other half of the same state — when it changes the draft is dropped so
 * both views agree.
 */
export function AssemblyField({ code, disabled, title, onChange }: AssemblyFieldProps): JSX.Element {
  const [draft, setDraft] = useState<string | null>(null);
  const [errors, setErrors] = useState<AsmError[]>([]);

  const codeKey = code.join(',');
  const lastEmitted = useRef(codeKey);

  useEffect(() => {
    if (codeKey !== lastEmitted.current) {
      lastEmitted.current = codeKey;
      setDraft(null);
      setErrors([]);
    }
  }, [codeKey]);

  useEffect(() => {
    if (disabled) {
      setDraft(null);
      setErrors([]);
    }
  }, [disabled]);

  const handleChange = (text: string) => {
    setDraft(text);
    const result = assemble(text);
    setErrors(result.errors);
    if (result.errors.length === 0) {
      lastEmitted.current = result.code.join(',');
      onChange(result.code);
    }
  };

  return (
    <label className={styles.field} title={title}>
      <span>Assembly (one instruction per line)</span>
      <textarea
        className={`${styles.textarea} ${styles.mono}`}
        spellCheck={false}
        disabled={disabled}
        value={draft ?? disassemble(code).join('\n')}
        onChange={(event) => handleChange(event.target.value)}
        onBlur={() => {
          if (errors.length === 0) {
            setDraft(null);
          }
        }}
      />
      <span className={errors.length > 0 ? styles.errorLines : styles.hint}>
        {errors.map((error, index) => (
          <span key={`${error.line}-${error.token}-${index}`}>
            Line {error.line}: <span className={styles.mono}>&quot;{error.token}&quot;</span> — {error.message}
            {error.suggestions.length > 0 ? ` — did you mean ${error.suggestions.join(', ')}?` : ''}
          </span>
        ))}
      </span>
    </label>
  );
}
