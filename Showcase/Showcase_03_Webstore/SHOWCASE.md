# 🛒 Showcase 03: E-Commerce Webstore Multi-DB (Internal Contract DB + External Shop DB)

This showcase demonstrates the **Multi-Database Architecture** (**Internal Database** + **Attached External Database**) combined with **Hybrid Search + GraphRAG + Dynamic Text-to-SurrealQL**:

1. **Internal Database (`rag_namespace / knowledge_base`)**:
   - **6 Unstructured PDF Contracts** (`documents/`):
     - **Supplier Contracts (`Liefervertraege/`)**:
       - `Liefervertrag_NordicAudio_Kopfhoerer_2026.pdf`: **12% volume kickback rebate** once a product reaches **200 sold units** across March & April 2026; maximum allowed customer return rate **4.0%** (otherwise a **2,500 EUR quality penalty** applies); minimum safety stock for `ART-1001` >= 25 units.
       - `Liefervertrag_AlpenTech_SmartHome_Espresso_2026.pdf`: **10% volume bonus** once a product reaches **80 sold units** across March & April 2026; mandatory safety stock for `ART-2001` (Barista Pro Espresso Machine) >= **15 units**; **8% purchase price deduction** if express restocking is delayed by >= 9 days.
     - **Marketing & Advertising Contracts (`Werbevertraege/`)**:
       - `Werbevertrag_PulseReach_Influencer_Kampagne_2026.pdf`: Monthly base retainer **8,500 EUR/month** + **8% revenue commission** on campaign `'PulseReach-Spring'` (discount code `SPRING26`). **April 2026 Malus Clause**: If April campaign revenue falls below the guaranteed minimum of **45,000 EUR**, the webstore receives an immediate **3,000 EUR penalty credit** (reducing the April retainer to 5,500 EUR) and the commission rate is halved from 8% to **4%**.
       - `Werbevertrag_AdVantage_GoogleShopping_2026.pdf`: Fixed monthly retainer **6,200 EUR/month** for `'Google-Shopping-DE'`. **April 2026 Bonus Clause**: If April campaign revenue exceeds **50,000 EUR**, AdVantage receives a **1,500 EUR performance bonus** (total April payout: 7,700 EUR).
     - **Warehouse Lease Agreements (`Mietvertraege_Lager/`)**:
       - `Mietvertrag_Fulfillment_Lager_Hamburg_2026.pdf`: Base rent **4,200 EUR/month** including up to **120 occupied pallets** (`25.00 EUR` per additional pallet above 120). **Fire Safety & Lithium Battery Cap**: Maximum **600 units containing lithium-ion batteries** (`contains_lithium_battery = true`) allowed simultaneously (otherwise a **5,000 EUR fire safety penalty** applies unless excess units are relocated to Munich!).
       - `Mietvertrag_CityHub_Muenchen_Retourenlager_2026.pdf`: Base rent **2,800 EUR/month** including up to **80 occupied pallets** (`20.00 EUR` per extra pallet); VdS hazmat certification for up to **1,500 lithium-battery units**; **6.50 EUR** refurbishment fee per returned unit.
   - **Internal SQL Table (`internal_contract_ledger`)**:
     - Stores internal contract master data, monthly fixed costs, supplier delivery delays (`AlpenTech GmbH: 11 days delay`), and contract status.

2. **Attached External Webstore Database (`webstore_external / shop_erp`)**:
   - **`shop_product` (12 Webstore Products)**:
     - Contains `sku`, `product_name`, `category`, `supplier_name`, `purchase_price_eur`, `retail_price_eur`, `margin_eur`, `stock_units`, `min_stock_sla`, `occupied_pallets`, `warehouse_location`, `contains_lithium_battery`, and `return_rate_percent`.
   - **`shop_sale` (14 Sales Order Batches across March & April 2026)**:
     - Contains `order_id`, `order_date`, `month`, `sku`, `product_name`, `quantity`, `unit_price_eur`, `purchase_cost_eur`, `revenue_eur`, `profit_eur`, `discount_code`, `marketing_campaign`, `customer_region`, and `order_status`.

---

## 🚀 Start Showcase 03

```bash
./setup.sh --showcase 3
# (Alternatively: ./setup.sh --showcase webstore)
```

---

## 🧪 Multi-DB & Multi-Hop Test Prompts (Copy & Paste for the Chat UI)

### 1. Marketing Contracts (Internal PDF + Internal DB) $\times$ April Campaign Revenue (External DB `shop_sale`)
> **Prompt (EN):**
> *"Check our advertising contracts with PulseReach Media and AdVantage Digital for April 2026 against the actual sales figures in the webstore database: Which agency met its sales target in April, where does the malus clause apply, and what exact amount must we transfer to each agency for April?"*
> **Prompt (DE):**
> *„Prüfe unsere Werbeverträge mit PulseReach Media und AdVantage Digital für April 2026 anhand der echten Verkaufszahlen aus der Webstore-Datenbank: Welche Agentur hat ihr Umsatzziel im April erreicht, wo greift die Malus-Klausel, und wie viel müssen wir beiden Agenturen für April exakt überweisen?“*

**Expected Deterministic Result (Hop 1 PDF + Hop 2A Internal DB + Hop 2B External DB):**
- **`PulseReach-Spring` (PulseReach Media GmbH)**:
  - April revenue in `shop_sale`: **32,400.00 EUR** (from 3 order batches: 14,900 + 9,500 + 8,000 EUR) -> **below the guaranteed 45,000 EUR minimum!**
  - **Malus applies**: Base retainer is reduced by **3,000 EUR** from 8,500 EUR to **5,500.00 EUR**; commission is halved from 8% to **4%** (`4% of 32,400 EUR = 1,296.00 EUR`).
  - **Total April payout to PulseReach**: **6,796.00 EUR** (instead of the regular 11,092.00 EUR).
- **`Google-Shopping-DE` (AdVantage Digital AG)**:
  - April revenue in `shop_sale`: **58,600.00 EUR** (22,400 + 24,200 + 12,000 EUR) -> **above the 50,000 EUR bonus threshold!**
  - **Bonus applies**: Base retainer **6,200.00 EUR** + **1,500.00 EUR performance bonus** = **7,700.00 EUR**.

---

### 2. Warehouse Leases (Internal PDF + Internal DB) $\times$ Pallets & Lithium Batteries (External DB `shop_product`)
> **Prompt (EN):**
> *"Check our lease agreements for the Hamburg-Billbrook (WH-HAM) and Munich-East (WH-MUC) warehouses against current inventory in the webstore database: How many pallets and how many lithium-ion battery units are currently stored in each warehouse? What extra rent or penalty applies in Hamburg, and can we relocate the excess battery units to Munich?"*
> **Prompt (DE):**
> *„Prüfe unseren Mietvertrag für das Fulfillment-Lager Hamburg-Billbrook (WH-HAM) und München-Ost (WH-MUC): Wie viele Paletten und wie viele Artikel mit Lithium-Ionen-Akku lagern aktuell in beiden Hallen? Welche Zusatzmiete oder Vertragsstrafe droht in Hamburg, und können wir die überzähligen Akku-Artikel nach München umlagern?“*

**Expected Deterministic Result:**
- **Hamburg-Billbrook Warehouse (`WH-HAM`)**:
  - **Pallet Utilization**: **148 occupied pallets** vs. **120 included pallets** -> **28 extra pallets × 25.00 EUR = 700.00 EUR surcharge** (total Hamburg rent: **4,900.00 EUR/month**).
  - **Lithium-Ion Battery Stock**: **740 units** (`ART-1001`: 190 + `ART-1003`: 260 + `ART-3001`: 290) -> **140 units above the 600-unit fire safety cap!** A **5,000 EUR fire safety penalty** threatens unless relocated.
- **Munich-East Warehouse (`WH-MUC`)**:
  - **Pallet Utilization**: **54 of 80 included pallets** occupied (**26 pallet slots free**, 0 EUR surcharge).
  - **Lithium-Ion Battery Stock**: Only **190 of 1,500 permitted units** (`ART-2005`) -> All **140 excess battery units from Hamburg** can be relocated to Munich-East immediately without incurring extra rent!

---

### 3. Supplier Contracts (Internal PDF + Internal DB) $\times$ Kickback Rebates, Return Rates & Safety Stock (External DB)
> **Prompt (EN):**
> *"Analyze our supplier contracts with NordicAudio AB and AlpenTech GmbH against the webstore product and sales tables: Which products qualify for volume kickbacks or bonuses across March and April, which product violates the maximum return rate SLA, and where is the minimum safety stock breached?"*
> **Prompt (DE):**
> *„Analysiere unsere Lieferverträge mit NordicAudio AB und AlpenTech GmbH: Für welche Artikel steht uns aus den Verkäufen von März und April der Volumen-Kickback bzw. Bonus zu, welcher Artikel verstößt gegen die maximale Retourenquote, und wo ist der Mindestbestand unterschritten?“*

**Expected Deterministic Result:**
- **Volume Kickbacks & Bonuses (March + April 2026 in `shop_sale`)**:
  - `ART-1001` (*NordicAudio ANC Pro Headphones*): **245 units sold** (`65 + 45 + 60 + 75 >= 200 units`) with a total purchase value of **26,950.00 EUR** -> **12% kickback rebate = 3,234.00 EUR** from `NordicAudio AB`.
  - `ART-2003` (*AlpenTech Smart-Thermostat V3*): **110 units sold** (`45 + 65 >= 80 units`) with a total purchase value of **4,620.00 EUR** -> **10% volume bonus = 462.00 EUR** from `AlpenTech GmbH`.
- **Quality SLA Violation (Return Rate > 4.0% in `shop_product`)**:
  - `ART-1003` (*NordicAudio TrueWireless Earbuds Mini*) has a **7.4% return rate** (> 4.0% threshold) -> **2,500.00 EUR quality penalty** + free batch replacement by `NordicAudio AB`.
- **Safety Stock Violation & Delivery Delay**:
  - `ART-2001` (*AlpenTech Barista Pro Espresso Machine*) has only **6 units in stock** (mandatory minimum: **15 units**). Because `AlpenTech GmbH` is **11 days late** on the express restock order (>= 9 days), we are entitled to an **8% purchase price deduction**.
