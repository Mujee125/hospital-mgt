/// Message text builders for every WhatsApp notification type the system
/// sends. Pure string formatting — no I/O, no Tauri/sqlx dependencies —
/// kept separate so templates can be edited/tested without touching the
/// automation or logging code.
pub fn build_appointment_booked_msg(
    clinic: &str,
    patient_name: &str,
    doctor_name: &str,
    date: &str,
    time: &str,
) -> String {
    format!(
        "Hello {patient}! 👋\n\n\
        Your appointment at *{clinic}* has been *booked successfully*. ✅\n\n\
        📋 *Details:*\n\
        • Doctor: Dr. {doctor}\n\
        • Date: {date}\n\
        • Time: {time}\n\n\
        Please arrive 10 minutes early. Reply CANCEL to cancel.\n\n\
        _{clinic}_",
        patient = patient_name,
        clinic = clinic,
        doctor = doctor_name,
        date = date,
        time = time,
    )
}

pub fn build_appointment_confirmed_msg(
    clinic: &str,
    patient_name: &str,
    doctor_name: &str,
    date: &str,
    time: &str,
) -> String {
    format!(
        "Hello {patient}! 🎉\n\n\
        Your appointment at *{clinic}* has been *confirmed* by Dr. {doctor}. ✅\n\n\
        📅 Date: {date}\n\
        ⏰ Time: {time}\n\n\
        We look forward to seeing you!\n\
        _{clinic}_",
        patient = patient_name,
        clinic = clinic,
        doctor = doctor_name,
        date = date,
        time = time,
    )
}

pub fn build_appointment_cancelled_msg(
    clinic: &str,
    patient_name: &str,
    doctor_name: &str,
    date: &str,
    time: &str,
) -> String {
    format!(
        "Hello {patient},\n\n\
        We regret to inform you that your appointment at *{clinic}* has been *cancelled*.\n\n\
        ❌ Cancelled appointment:\n\
        • Doctor: Dr. {doctor}\n\
        • Date: {date}\n\
        • Time: {time}\n\n\
        Please contact us to reschedule. We apologise for any inconvenience.\n\
        _{clinic}_",
        patient = patient_name,
        clinic = clinic,
        doctor = doctor_name,
        date = date,
        time = time,
    )
}

pub fn build_reminder_msg(
    clinic: &str,
    patient_name: &str,
    doctor_name: &str,
    time: &str,
) -> String {
    format!(
        "⏰ *Appointment Reminder* — {clinic}\n\n\
        Hello {patient}! Your appointment with Dr. {doctor} is in *1 hour* at {time}.\n\n\
        Please be on time. See you soon! 🏥\n\
        _{clinic}_",
        clinic = clinic,
        patient = patient_name,
        doctor = doctor_name,
        time = time,
    )
}

pub fn build_daily_digest_msg(
    clinic: &str,
    date: &str,
    appointments: &[(String, String, String, String)], // (time, patient, doctor, status)
) -> String {
    let mut lines = format!(
        "🏥 *{clinic} — Daily Schedule*\n📅 {date}\n\n",
        clinic = clinic,
        date = date,
    );

    if appointments.is_empty() {
        lines.push_str("No appointments scheduled for today.");
    } else {
        lines.push_str(&format!("*{} appointment(s) today:*\n\n", appointments.len()));
        for (time, patient, doctor, status) in appointments {
            let icon = match status.as_str() {
                "confirmed" => "✅",
                "completed" => "🏁",
                "cancelled" => "❌",
                "no-show" => "⚠️",
                _ => "🔵",
            };
            lines.push_str(&format!("{} *{}* — {} with Dr. {}\n", icon, time, patient, doctor));
        }
    }

    lines.push_str("\n_Sent by VitalFlow HMS_");
    lines
}
