import { useSimContext } from '../context/SimContext';
import { ControlsTab } from './controls/ControlsTab';
import { InspectorTab } from './inspector/InspectorTab';
import styles from './Sidebar.module.css';

export function Sidebar(): JSX.Element {
  const { setSidebarOpen, setSidebarTab, state } = useSimContext();

  if (!state.sidebarOpen) {
    return (
      <aside className={styles.collapsedRail}>
        <button className={styles.toggle} type="button" onClick={() => setSidebarOpen(true)}>
          Open Sidebar
        </button>
      </aside>
    );
  }

  return (
    <aside className={styles.sidebar}>
      <div className={styles.topbar}>
        <button className={styles.toggle} type="button" onClick={() => setSidebarOpen(false)}>
          Collapse
        </button>
      </div>
      <div className={styles.tabs}>
        <button
          className={state.sidebarTab === 'controls' ? styles.tabActive : styles.tab}
          type="button"
          onClick={() => setSidebarTab('controls')}
        >
          Controls
        </button>
        <button
          className={state.sidebarTab === 'inspector' ? styles.tabActive : styles.tab}
          type="button"
          onClick={() => setSidebarTab('inspector')}
        >
          Inspector
        </button>
      </div>
      <div className={styles.body}>
        {state.sidebarTab === 'controls' ? <ControlsTab /> : <InspectorTab />}
      </div>
    </aside>
  );
}
