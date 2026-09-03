import { Printer } from "lucide-react";
import { Button } from "@/components/ui/button";

/**
 * Printable appointment receipt.
 *
 * Print path: a plain `window.print()` button. This relies on the
 * thermal printer being installed as a normal Windows printer (the kind
 * that shows up in any app's Print dialog) — most 80mm POS receipt
 * printers ship with exactly this kind of driver for precisely this
 * reason. If your printer turns out to be ESC/POS-only with no Windows
 * driver, this approach won't reach it and a different integration would
 * be needed — confirm with a real test print before relying on this.
 *
 * The `@media print` rules below size the receipt for 80mm thermal paper
 * specifically (not A4/letter) using physical units (mm), and hide
 * everything else on the page — the dialog overlay, the rest of the
 * app shell — so only the receipt itself ends up on paper.
 */

export interface ReceiptData {
  clinicName: string;
  appointmentId: number;
  patientName: string;
  patientPhone: string;
  doctorName: string;
  doctorSpecialization: string;
  date: string; // already formatted, e.g. "21 Jun 2026"
  time: string; // already formatted, e.g. "10:30 AM"
  durationMinutes: number;
  reason: string | null;
  status: string;
  bookedAt: string; // formatted timestamp
}

export function Receipt({ data }: { data: ReceiptData }) {
  const handlePrint = () => window.print();

  return (
    <div>
      {/* Print-only styling: scoped to this component via the
          .receipt-print-area marker class, so printing from elsewhere in
          the app is never accidentally affected by these rules. */}
      <style>{`
        @media print {
          body * { visibility: hidden; }
          .receipt-print-area, .receipt-print-area * { visibility: visible; }
          .receipt-print-area {
            position: fixed;
            left: 0;
            top: 0;
            transform: none;
            width: 80mm;
          }
          .receipt-no-print { display: none !important; }
          @page {
            size: 80mm auto;
            margin: 2mm;
          }
        }
      `}</style>

      <div className="receipt-print-area font-mono text-[11px] leading-snug bg-white text-black p-3 mx-auto max-w-[320px] border border-dashed border-border/60 rounded">
        <div className="text-center space-y-0.5 mb-2">
          <p className="font-bold text-sm">{data.clinicName}</p>
          <p className="text-[10px]">Appointment Receipt</p>
        </div>
        <div className="border-t border-dashed border-black/30 my-2" />

        <Row label="Receipt #" value={`A-${data.appointmentId.toString().padStart(6, "0")}`} />
        <Row label="Booked" value={data.bookedAt} />
        <div className="border-t border-dashed border-black/30 my-2" />

        <Row label="Patient" value={data.patientName} />
        <Row label="Phone" value={data.patientPhone} />
        <div className="border-t border-dashed border-black/30 my-2" />

        <Row label="Doctor" value={`Dr. ${data.doctorName}`} />
        <Row label="Dept." value={data.doctorSpecialization} />
        <Row label="Date" value={data.date} />
        <Row label="Time" value={data.time} />
        <Row label="Duration" value={`${data.durationMinutes} min`} />
        {data.reason ? <Row label="Reason" value={data.reason} /> : null}
        <Row label="Status" value={data.status.toUpperCase()} />

        <div className="border-t border-dashed border-black/30 my-2" />
        <p className="text-center text-[10px] mt-2">
          Please arrive 10 minutes early.
          <br />
          Thank you for choosing {data.clinicName}.
        </p>
      </div>

      <div className="receipt-no-print flex justify-center mt-4">
        <Button onClick={handlePrint} className="gap-2">
          <Printer className="h-4 w-4" />
          Print Receipt
        </Button>
      </div>
    </div>
  );
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex justify-between gap-2">
      <span className="opacity-70">{label}:</span>
      <span className="font-semibold text-right break-words">{value}</span>
    </div>
  );
}
