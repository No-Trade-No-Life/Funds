import { StrictMode, useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';
import './styles.css';

type FundRecord = {
  account_id: string;
  description: string;
  state: {
    total_assets: number;
    summary: {
      total_share: number;
      unit_price: number;
      total_profit: number;
    };
    investors: Record<string, { share: number; deposit: number }>;
  };
  events: unknown[];
};

function App() {
  const [funds, setFunds] = useState<FundRecord[]>([]);
  const [message, setMessage] = useState('Loading funds...');

  useEffect(() => {
    void loadFunds();
  }, []);

  async function loadFunds() {
    const response = await fetch('/funds');
    const records = (await response.json()) as FundRecord[];
    setFunds(records);
    setMessage(records.length === 0 ? 'No funds yet. Create one through the API.' : 'Funds loaded.');
  }

  const totalAssets = funds.reduce((sum, fund) => sum + fund.state.total_assets, 0);
  const totalInvestors = funds.reduce((sum, fund) => sum + Object.keys(fund.state.investors).length, 0);

  return (
    <main className="shell">
      <section className="hero">
        <p className="eyebrow">Fund Operations</p>
        <h1>Funds dashboard</h1>
        <p className="intro">A Vite, TypeScript, and React frontend served through the Rust gateway.</p>
        <div className="actions">
          <a href="/docs">Open API Docs</a>
          <button type="button" onClick={() => void loadFunds()}>
            Refresh
          </button>
        </div>
      </section>

      <section className="metrics" aria-label="Fund summary metrics">
        <Metric label="Funds" value={funds.length.toString()} />
        <Metric label="Investors" value={totalInvestors.toString()} />
        <Metric label="Assets" value={formatMoney(totalAssets)} />
      </section>

      <section className="panel">
        <div className="panelHeader">
          <h2>Managed funds</h2>
          <span>{message}</span>
        </div>
        <div className="fundList">
          {funds.map((fund) => (
            <article className="fundCard" key={fund.account_id}>
              <div>
                <h3>{fund.account_id}</h3>
                <p>{fund.description}</p>
              </div>
              <dl>
                <dt>Unit price</dt>
                <dd>{fund.state.summary.unit_price.toFixed(4)}</dd>
                <dt>Total share</dt>
                <dd>{fund.state.summary.total_share.toFixed(2)}</dd>
                <dt>Events</dt>
                <dd>{fund.events.length}</dd>
              </dl>
            </article>
          ))}
        </div>
      </section>
    </main>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <article className="metric">
      <span>{label}</span>
      <strong>{value}</strong>
    </article>
  );
}

function formatMoney(value: number) {
  return new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency: 'USD',
    maximumFractionDigits: 0,
  }).format(value);
}

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
