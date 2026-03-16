import { GridCanvas } from './components/GridCanvas';
import { MetricsDrawer } from './components/MetricsDrawer';
import { Sidebar } from './components/Sidebar';
import { StatusBar } from './components/StatusBar';
import { useSimContext } from './context/SimContext';
import styles from './App.module.css';

export function App(): JSX.Element {
  const { state } = useSimContext();

  return (
    <div className={styles.app}>
      <main className={state.sidebarOpen ? styles.main : styles.mainCollapsed}>
        <GridCanvas />
        <Sidebar />
      </main>
      <MetricsDrawer />
      <StatusBar />
    </div>
  );
}
