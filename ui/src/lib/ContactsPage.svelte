<script lang="ts">
  import CalendarDays from "lucide-svelte/icons/calendar-days";
  import Download from "lucide-svelte/icons/download";
  import FileJson from "lucide-svelte/icons/file-json";
  import ImagePlus from "lucide-svelte/icons/image-plus";
  import Mail from "lucide-svelte/icons/mail";
  import MapPin from "lucide-svelte/icons/map-pin";
  import Pencil from "lucide-svelte/icons/pencil";
  import Phone from "lucide-svelte/icons/phone";
  import Plus from "lucide-svelte/icons/plus";
  import RefreshCw from "lucide-svelte/icons/refresh-cw";
  import Search from "lucide-svelte/icons/search";
  import Server from "lucide-svelte/icons/server";
  import Star from "lucide-svelte/icons/star";
  import Trash2 from "lucide-svelte/icons/trash-2";
  import Upload from "lucide-svelte/icons/upload";
  import UsersRound from "lucide-svelte/icons/users-round";
  import X from "lucide-svelte/icons/x";
  import { onMount, tick } from "svelte";
  import { SvelteMap, SvelteSet } from "svelte/reactivity";
  import AnimatedList from "$lib/components/AnimatedList.svelte";
  import {
    createContact,
    createContactDavSource,
    deleteContact,
    deleteContactDavSource,
    deleteContactPhoto,
    exportContacts,
    fetchContacts,
    importContacts,
    syncContactDavSource,
    updateContact,
    updateContactPhoto,
    type Contact,
    type ContactDavSource,
    type ContactInput,
    type ContactsResponse,
  } from "$lib/api";

  type ContactFilter = "active" | "favorites" | "archived";
  type ContactSort = "name" | "updated" | "newest" | "company" | "birthday";

  interface BirthdayParts {
    year: number | null;
    month: number;
    day: number;
  }

  const contactCollator = new Intl.Collator("en", {
    numeric: true,
    sensitivity: "base",
  });

  let contactsData = $state.raw<ContactsResponse>({
    contacts: [],
    dav_sources: [],
    secret_storage_enabled: false,
  });
  let loading = $state(true);
  let pageError = $state("");
  let query = $state("");
  let filter = $state<ContactFilter>("active");
  let sort = $state<ContactSort>("name");
  let tagFilters = $state<string[]>([]);
  let selected = $state.raw<Contact | null>(null);
  let detailDialog = $state<HTMLDialogElement>();
  let editorDialog = $state<HTMLDialogElement>();
  let davDialog = $state<HTMLDialogElement>();
  let importInput = $state<HTMLInputElement>();
  let editorNameInput = $state<HTMLInputElement>();
  let editingId = $state<string | null>(null);
  let contactDraft = $state<ContactInput>(emptyContact());
  let birthdayYearUnknown = $state(false);
  let birthdayMonthDay = $state("");
  let tagText = $state("");
  let formError = $state("");
  let saving = $state(false);
  let busyId = $state("");
  let deleteId = $state("");
  let sourceDeleteId = $state("");
  let davName = $state("");
  let davUrl = $state("");
  let davUsername = $state("");
  let davPassword = $state("");
  let davError = $state("");
  let importStatus = $state("");
  let contactPhotoFile = $state<File | null>(null);
  let contactPhotoPreview = $state("");
  let contactPhotoReset = $state(false);
  let contactPhotoRevision = $state(Date.now());

  let availableTags = $derived.by(() => {
    const tags = new SvelteMap<
      string,
      { key: string; label: string; count: number }
    >();
    for (const contact of contactsData.contacts) {
      for (const label of contact.tags) {
        const key = label.trim().toLowerCase();
        if (!key) continue;
        const existing = tags.get(key);
        if (existing) existing.count += 1;
        else tags.set(key, { key, label: label.trim(), count: 1 });
      }
    }
    return [...tags.values()].sort(
      (left, right) =>
        right.count - left.count || left.label.localeCompare(right.label),
    );
  });

  let filteredContacts = $derived.by(() => {
    const needle = query.trim().toLowerCase();
    return contactsData.contacts
      .filter((contact) => {
        if (filter === "active" && contact.archived) return false;
        if (filter === "favorites" && (!contact.favorite || contact.archived))
          return false;
        if (filter === "archived" && !contact.archived) return false;
        if (
          tagFilters.length > 0 &&
          !tagFilters.every((tag) =>
            contact.tags.some((candidate) => candidate.toLowerCase() === tag),
          )
        )
          return false;
        if (!needle) return true;
        return [
          displayName(contact),
          contact.nickname,
          contact.company,
          contact.job_title,
          ...contact.tags,
          ...contact.emails.map((method) => method.value),
          ...contact.phones.map((method) => method.value),
        ].some((value) => value.toLowerCase().includes(needle));
      })
      .sort(compareContacts);
  });

  let favoriteCount = $derived(
    contactsData.contacts.filter(
      (contact) => contact.favorite && !contact.archived,
    ).length,
  );
  let upcomingDates = $derived.by(() =>
    contactsData.contacts
      .filter((contact) => !contact.archived)
      .flatMap((contact) => contactDates(contact))
      .sort((left, right) => left.next.valueOf() - right.next.valueOf())
      .slice(0, 5),
  );

  onMount(() => {
    void loadContacts();
  });

  async function loadContacts() {
    loading = true;
    pageError = "";
    try {
      contactsData = await fetchContacts();
      if (selected) {
        selected =
          contactsData.contacts.find(
            (contact) => contact.id === selected?.id,
          ) ?? null;
      }
    } catch (reason: unknown) {
      pageError =
        reason instanceof Error ? reason.message : "Unable to load contacts";
    } finally {
      loading = false;
    }
  }

  function captureDetailDialog(node: HTMLDialogElement) {
    detailDialog = node;
    return () => (detailDialog = undefined);
  }

  function captureEditorDialog(node: HTMLDialogElement) {
    editorDialog = node;
    return () => (editorDialog = undefined);
  }

  function captureDavDialog(node: HTMLDialogElement) {
    davDialog = node;
    return () => (davDialog = undefined);
  }

  function captureImportInput(node: HTMLInputElement) {
    importInput = node;
    return () => (importInput = undefined);
  }

  function captureEditorName(node: HTMLInputElement) {
    editorNameInput = node;
    return () => (editorNameInput = undefined);
  }

  async function openDetail(contact: Contact) {
    selected = contact;
    deleteId = "";
    detailDialog?.showModal();
    await tick();
  }

  async function openCreate() {
    clearContactPhotoDraft();
    editingId = null;
    contactDraft = emptyContact();
    birthdayYearUnknown = false;
    birthdayMonthDay = "";
    tagText = "";
    formError = "";
    editorDialog?.showModal();
    await tick();
    editorNameInput?.focus();
  }

  async function openEdit(contact: Contact) {
    clearContactPhotoDraft();
    editingId = contact.id;
    contactDraft = contactInput(contact);
    const birthday = parseBirthday(contact.birthday);
    birthdayYearUnknown = birthday?.year === null;
    birthdayMonthDay =
      birthday?.year === null ? formatBirthdayMonthDay(birthday) : "";
    tagText = "";
    formError = "";
    detailDialog?.close();
    editorDialog?.showModal();
    await tick();
    editorNameInput?.focus();
  }

  async function saveContact(event: SubmitEvent) {
    event.preventDefault();
    if (saving) return;
    formError = "";
    if (!commitDraftTags()) return;
    if (birthdayYearUnknown) {
      const rawBirthday = birthdayMonthDay.trim();
      const birthday = rawBirthday ? parseBirthday(`--${rawBirthday}`) : null;
      if (rawBirthday && !birthday) {
        formError = "Enter a valid birthday as MM-DD.";
        return;
      }
      contactDraft.birthday = birthday
        ? `--${formatBirthdayMonthDay(birthday)}`
        : null;
    }
    saving = true;
    try {
      let saved = editingId
        ? await updateContact(editingId, $state.snapshot(contactDraft))
        : await createContact($state.snapshot(contactDraft));
      try {
        if (contactPhotoFile) {
          await updateContactPhoto(saved.id, contactPhotoFile);
          saved = { ...saved, has_photo: true };
          contactPhotoRevision = Date.now();
        } else if (contactPhotoReset && saved.has_photo) {
          await deleteContactPhoto(saved.id);
          saved = { ...saved, has_photo: false };
          contactPhotoRevision = Date.now();
        }
      } catch (reason: unknown) {
        editingId = saved.id;
        selected = saved;
        contactsData = {
          ...contactsData,
          contacts: contactsData.contacts.some(
            (contact) => contact.id === saved.id,
          )
            ? contactsData.contacts.map((contact) =>
                contact.id === saved.id ? saved : contact,
              )
            : [...contactsData.contacts, saved],
        };
        formError =
          reason instanceof Error
            ? `Contact saved, but the photo failed: ${reason.message}`
            : "Contact saved, but the photo could not be updated.";
        return;
      }
      contactsData = {
        ...contactsData,
        contacts: editingId
          ? contactsData.contacts.map((contact) =>
              contact.id === saved.id ? saved : contact,
            )
          : [...contactsData.contacts, saved],
      };
      selected = saved;
      clearContactPhotoDraft();
      editorDialog?.close();
      detailDialog?.showModal();
    } catch (reason: unknown) {
      formError =
        reason instanceof Error ? reason.message : "Unable to save contact";
    } finally {
      saving = false;
    }
  }

  function toggleTagFilter(tag: string) {
    tagFilters = tagFilters.includes(tag)
      ? tagFilters.filter((candidate) => candidate !== tag)
      : [...tagFilters, tag];
  }

  function tagControlId(tag: string, index: number) {
    const slug = tag
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-|-$/g, "");
    return `contact-tag-filter-${index}-${slug || "tag"}`;
  }

  function commitDraftTags() {
    const candidates = tagText
      .split(",")
      .map((tag) => tag.trim())
      .filter(Boolean);
    if (candidates.length === 0) return true;

    const existing = new SvelteSet(
      contactDraft.tags.map((tag) => tag.toLowerCase()),
    );
    const next = [...contactDraft.tags];
    for (const candidate of candidates) {
      if ([...candidate].length > 40) {
        formError = "Tags must be 40 characters or fewer.";
        return false;
      }
      const key = candidate.toLowerCase();
      if (existing.has(key)) continue;
      if (next.length >= 30) {
        formError = "A contact can have up to 30 tags.";
        return false;
      }
      existing.add(key);
      next.push(candidate);
    }
    contactDraft.tags = next;
    tagText = "";
    formError = "";
    return true;
  }

  function removeDraftTag(tag: string) {
    contactDraft.tags = contactDraft.tags.filter(
      (candidate) => candidate !== tag,
    );
  }

  function handleTagKeydown(event: KeyboardEvent) {
    if (event.key === "Enter" || event.key === ",") {
      event.preventDefault();
      commitDraftTags();
    } else if (
      event.key === "Backspace" &&
      tagText.length === 0 &&
      contactDraft.tags.length > 0
    ) {
      contactDraft.tags = contactDraft.tags.slice(0, -1);
    }
  }

  async function toggleFavorite(contact: Contact) {
    if (busyId) return;
    busyId = contact.id;
    pageError = "";
    try {
      const saved = await updateContact(contact.id, {
        ...contactInput(contact),
        favorite: !contact.favorite,
      });
      contactsData = {
        ...contactsData,
        contacts: contactsData.contacts.map((item) =>
          item.id === saved.id ? saved : item,
        ),
      };
      selected = saved;
    } catch (reason: unknown) {
      pageError =
        reason instanceof Error ? reason.message : "Unable to update contact";
    } finally {
      busyId = "";
    }
  }

  async function removeContact(contact: Contact) {
    if (busyId) return;
    if (deleteId !== contact.id) {
      deleteId = contact.id;
      return;
    }
    busyId = contact.id;
    try {
      await deleteContact(contact.id);
      contactsData = {
        ...contactsData,
        contacts: contactsData.contacts.filter(
          (item) => item.id !== contact.id,
        ),
      };
      selected = null;
      detailDialog?.close();
      deleteId = "";
    } catch (reason: unknown) {
      pageError =
        reason instanceof Error ? reason.message : "Unable to remove contact";
    } finally {
      busyId = "";
    }
  }

  async function downloadExport() {
    pageError = "";
    try {
      const blob = await exportContacts();
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement("a");
      anchor.href = url;
      anchor.download = "pandan-contacts.json";
      anchor.click();
      URL.revokeObjectURL(url);
    } catch (reason: unknown) {
      pageError =
        reason instanceof Error ? reason.message : "Unable to export contacts";
    }
  }

  async function handleImport(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    pageError = "";
    importStatus = "Importing contacts…";
    try {
      const payload: unknown = JSON.parse(await file.text());
      const format =
        payload &&
        typeof payload === "object" &&
        "format" in payload &&
        payload.format === "pandan-contacts"
          ? "pandan-json"
          : "monica-json";
      const result = await importContacts(format, payload);
      importStatus = `Imported ${result.imported} of ${result.total} contacts${result.skipped ? `; ${result.skipped} skipped` : ""}.`;
      await loadContacts();
    } catch (reason: unknown) {
      importStatus = "";
      pageError =
        reason instanceof Error
          ? reason.message
          : "Unable to import contact file";
    } finally {
      input.value = "";
    }
  }

  function openDav() {
    davName = "";
    davUrl = "";
    davUsername = "";
    davPassword = "";
    davError = "";
    sourceDeleteId = "";
    davDialog?.showModal();
  }

  async function addDavSource(event: SubmitEvent) {
    event.preventDefault();
    if (saving) return;
    saving = true;
    davError = "";
    try {
      const source = await createContactDavSource({
        name: davName.trim(),
        url: davUrl.trim(),
        username: davUsername.trim(),
        password: davPassword,
      });
      contactsData = {
        ...contactsData,
        dav_sources: [...contactsData.dav_sources, source],
      };
      davName = "";
      davUrl = "";
      davUsername = "";
      davPassword = "";
    } catch (reason: unknown) {
      davError =
        reason instanceof Error ? reason.message : "Unable to add DAV source";
    } finally {
      saving = false;
    }
  }

  async function syncDavSource(source: ContactDavSource) {
    if (busyId) return;
    busyId = source.id;
    davError = "";
    try {
      const result = await syncContactDavSource(source.id);
      contactsData = {
        ...contactsData,
        dav_sources: contactsData.dav_sources.map((item) =>
          item.id === result.source.id ? result.source : item,
        ),
      };
      importStatus = `Pulled ${result.imported} contacts from ${source.name}.`;
      await loadContacts();
    } catch (reason: unknown) {
      davError =
        reason instanceof Error ? reason.message : "Unable to sync DAV source";
      await loadContacts();
    } finally {
      busyId = "";
    }
  }

  async function removeDavSource(source: ContactDavSource) {
    if (busyId) return;
    if (sourceDeleteId !== source.id) {
      sourceDeleteId = source.id;
      return;
    }
    busyId = source.id;
    try {
      await deleteContactDavSource(source.id);
      contactsData = {
        ...contactsData,
        dav_sources: contactsData.dav_sources.filter(
          (item) => item.id !== source.id,
        ),
      };
      sourceDeleteId = "";
      await loadContacts();
    } catch (reason: unknown) {
      davError =
        reason instanceof Error
          ? reason.message
          : "Unable to remove DAV source";
    } finally {
      busyId = "";
    }
  }

  function emptyContact(): ContactInput {
    return {
      first_name: "",
      middle_name: "",
      last_name: "",
      nickname: "",
      pronouns: "",
      company: "",
      job_title: "",
      birthday: null,
      emails: [{ label: "personal", value: "" }],
      phones: [{ label: "mobile", value: "" }],
      addresses: [],
      important_dates: [],
      tags: [],
      relationship_context: "",
      notes: "",
      favorite: false,
      archived: false,
    };
  }

  function contactInput(contact: Contact): ContactInput {
    return {
      first_name: contact.first_name,
      middle_name: contact.middle_name,
      last_name: contact.last_name,
      nickname: contact.nickname,
      pronouns: contact.pronouns,
      company: contact.company,
      job_title: contact.job_title,
      birthday: contact.birthday,
      emails: contact.emails.map((method) => ({ ...method })),
      phones: contact.phones.map((method) => ({ ...method })),
      addresses: contact.addresses.map((address) => ({ ...address })),
      important_dates: contact.important_dates.map((date) => ({ ...date })),
      tags: [...contact.tags],
      relationship_context: contact.relationship_context,
      notes: contact.notes,
      favorite: contact.favorite,
      archived: contact.archived,
    };
  }

  function displayName(
    contact: Pick<
      Contact,
      "first_name" | "middle_name" | "last_name" | "nickname"
    >,
  ) {
    const name = [contact.first_name, contact.middle_name, contact.last_name]
      .filter(Boolean)
      .join(" ");
    return name || contact.nickname || "Unnamed contact";
  }

  function compareContacts(left: Contact, right: Contact) {
    if (sort === "updated") {
      return (
        new Date(right.updated_at).valueOf() -
          new Date(left.updated_at).valueOf() ||
        contactCollator.compare(displayName(left), displayName(right))
      );
    }
    if (sort === "newest") {
      return (
        new Date(right.created_at).valueOf() -
          new Date(left.created_at).valueOf() ||
        contactCollator.compare(displayName(left), displayName(right))
      );
    }
    if (sort === "company") {
      return (
        contactCollator.compare(
          left.company || displayName(left),
          right.company || displayName(right),
        ) || contactCollator.compare(displayName(left), displayName(right))
      );
    }
    if (sort === "birthday") {
      return (
        nextBirthdayValue(left) - nextBirthdayValue(right) ||
        contactCollator.compare(displayName(left), displayName(right))
      );
    }
    return contactCollator.compare(displayName(left), displayName(right));
  }

  function nextBirthdayValue(contact: Contact) {
    const parts = parseBirthday(contact.birthday);
    if (!parts) return Number.POSITIVE_INFINITY;
    const now = new Date();
    let birthday = new Date(now.getFullYear(), parts.month - 1, parts.day, 12);
    if (birthday < now) {
      birthday = new Date(
        now.getFullYear() + 1,
        parts.month - 1,
        parts.day,
        12,
      );
    }
    return birthday.valueOf();
  }

  function initials(contact: Contact) {
    return (
      [contact.first_name, contact.last_name]
        .filter(Boolean)
        .slice(0, 2)
        .map((part) => part[0]?.toUpperCase())
        .join("") ||
      contact.nickname.slice(0, 2).toUpperCase() ||
      "--"
    );
  }

  function contactPhotoUrl(contact: Contact) {
    return (
      "/api/contacts/" +
      encodeURIComponent(contact.id) +
      `/photo?v=${contactPhotoRevision}`
    );
  }

  function editingContact() {
    return editingId
      ? (contactsData.contacts.find((contact) => contact.id === editingId) ??
          null)
      : null;
  }

  function editorPhotoSource() {
    if (contactPhotoReset) return "";
    if (contactPhotoPreview) return contactPhotoPreview;
    const contact = editingContact();
    return contact?.has_photo ? contactPhotoUrl(contact) : "";
  }

  function draftInitials() {
    return (
      [contactDraft.first_name, contactDraft.last_name]
        .filter(Boolean)
        .slice(0, 2)
        .map((part) => part[0]?.toUpperCase())
        .join("") ||
      contactDraft.nickname.slice(0, 2).toUpperCase() ||
      "--"
    );
  }

  function clearContactPhotoDraft() {
    if (contactPhotoPreview.startsWith("blob:")) {
      URL.revokeObjectURL(contactPhotoPreview);
    }
    contactPhotoFile = null;
    contactPhotoPreview = "";
    contactPhotoReset = false;
  }

  function selectContactPhoto(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    if (
      !["image/jpeg", "image/png", "image/webp", "image/avif"].includes(
        file.type,
      )
    ) {
      formError = "Choose a JPEG, PNG, WebP, or AVIF image.";
      input.value = "";
      return;
    }
    if (file.size > 10 * 1024 * 1024) {
      formError = "Contact photos must be 10 MB or smaller.";
      input.value = "";
      return;
    }
    if (contactPhotoPreview.startsWith("blob:")) {
      URL.revokeObjectURL(contactPhotoPreview);
    }
    contactPhotoFile = file;
    contactPhotoPreview = URL.createObjectURL(file);
    contactPhotoReset = false;
    formError = "";
    input.value = "";
  }

  function removeContactPhotoDraft() {
    if (contactPhotoPreview.startsWith("blob:")) {
      URL.revokeObjectURL(contactPhotoPreview);
    }
    contactPhotoFile = null;
    contactPhotoPreview = "";
    contactPhotoReset = true;
    formError = "";
  }

  function sourceLabel(contact: Contact) {
    if (contact.source_kind === "carddav") {
      return (
        contactsData.dav_sources.find(
          (source) => source.id === contact.dav_source_id,
        )?.name ?? "CardDAV"
      );
    }
    return contact.source_kind === "monica" ? "Imported" : "Pandan";
  }

  function parseBirthday(value: string | null | undefined): BirthdayParts | null {
    if (!value) return null;
    const match = /^(?:(\d{4})-|--)(\d{2})-(\d{2})$/.exec(value);
    if (!match) return null;
    const year = match[1] ? Number(match[1]) : null;
    const month = Number(match[2]);
    const day = Number(match[3]);
    const testYear = year ?? 2000;
    const date = new Date(testYear, month - 1, day, 12);
    if (
      date.getFullYear() !== testYear ||
      date.getMonth() !== month - 1 ||
      date.getDate() !== day
    )
      return null;
    return { year, month, day };
  }

  function formatBirthdayMonthDay(birthday: BirthdayParts) {
    return `${String(birthday.month).padStart(2, "0")}-${String(
      birthday.day,
    ).padStart(2, "0")}`;
  }

  function toggleBirthdayYearUnknown() {
    if (!birthdayYearUnknown) {
      const birthday = parseBirthday(contactDraft.birthday);
      birthdayMonthDay = birthday ? formatBirthdayMonthDay(birthday) : "";
    } else {
      contactDraft.birthday = null;
    }
    birthdayYearUnknown = !birthdayYearUnknown;
  }

  function formatDate(value: string) {
    const birthday = parseBirthday(value);
    if (birthday?.year === null) {
      const date = new Date(2000, birthday.month - 1, birthday.day, 12);
      return `${new Intl.DateTimeFormat("en", {
        month: "short",
        day: "numeric",
      }).format(date)} · Year unknown`;
    }
    const date = new Date(`${value}T12:00:00`);
    return Number.isNaN(date.valueOf())
      ? value
      : new Intl.DateTimeFormat("en", {
          month: "short",
          day: "numeric",
          year: "numeric",
        }).format(date);
  }

  function formatTimestamp(value: string | null) {
    if (!value) return "Never synced";
    const date = new Date(value);
    return Number.isNaN(date.valueOf())
      ? value
      : new Intl.DateTimeFormat("en", {
          month: "short",
          day: "numeric",
          hour: "numeric",
          minute: "2-digit",
        }).format(date);
  }

  function contactDates(contact: Contact) {
    const values = contact.important_dates.map((date) => ({
      label: date.label,
      date: date.date,
    }));
    if (contact.birthday)
      values.push({ label: "Birthday", date: contact.birthday });
    return values.flatMap((date) => {
      const parts = parseBirthday(date.date);
      if (!parts) return [];
      const now = new Date();
      let next = new Date(now.getFullYear(), parts.month - 1, parts.day, 12);
      if (next < new Date(now.getFullYear(), now.getMonth(), now.getDate())) {
        next = new Date(
          now.getFullYear() + 1,
          parts.month - 1,
          parts.day,
          12,
        );
      }
      return [
        { contact, label: date.label, next, originalYear: parts.year },
      ];
    });
  }
</script>

<section class="contacts-page product-page" data-od-id="contacts-page">
  <header class="contacts-header page-header" data-od-id="contacts-heading">
    <div>
      <h2>$ contacts --people</h2>
      <p>
        A private directory for the people, context, and dates you want to
        remember.
      </p>
    </div>
    <div class="header-actions">
      <input
        class="sr-only"
        type="file"
        accept="application/json,.json"
        onchange={handleImport}
        {@attach captureImportInput}
      />
      <button
        class="ui-button ui-button--secondary"
        type="button"
        onclick={() => importInput?.click()}
        data-od-id="import-contacts"><Upload size={16} /> Import</button
      >
      <button
        class="ui-button ui-button--secondary"
        type="button"
        onclick={downloadExport}
        data-od-id="export-contacts"><Download size={16} /> Export</button
      >
      <button class="ui-button ui-button--secondary" type="button" onclick={openDav} data-od-id="manage-carddav"
        ><Server size={16} /> DAV</button
      >
      <button
        class="ui-button ui-button--primary"
        type="button"
        onclick={openCreate}
        data-od-id="add-contact"><Plus size={16} /> Add contact</button
      >
    </div>
  </header>

  {#if pageError}<p class="page-error" role="alert">{pageError}</p>{/if}
  {#if importStatus}<p class="import-status" role="status">
      <FileJson size={15} />
      {importStatus}
    </p>{/if}

  <div class="contact-metrics" data-od-id="contact-summary">
    <div>
      <span>Active people</span><strong
        >{contactsData.contacts.filter((contact) => !contact.archived)
          .length}</strong
      >
    </div>
    <div><span>Favorites</span><strong>{favoriteCount}</strong></div>
    <div>
      <span>Upcoming dates</span><strong>{upcomingDates.length}</strong>
    </div>
    <div>
      <span>DAV resources</span><strong
        >{contactsData.dav_sources.length}</strong
      >
    </div>
  </div>

  <div class="contacts-toolbar" data-od-id="contacts-toolbar">
    <label class="search-field">
      <Search size={16} aria-hidden="true" />
      <span class="sr-only">Search contacts</span>
      <input
        type="search"
        bind:value={query}
        placeholder="Search name, company, tag, email, or phone…"
        data-od-id="contact-search"
      />
    </label>
    <div class="filter-group" aria-label="Contact view">
      <button
        class:active={filter === "active"}
        type="button"
        onclick={() => (filter = "active")}>Active</button
      >
      <button
        class:active={filter === "favorites"}
        type="button"
        onclick={() => (filter = "favorites")}>Favorites</button
      >
      <button
        class:active={filter === "archived"}
        type="button"
        onclick={() => (filter = "archived")}>Archived</button
      >
    </div>
    <label class="sort-field">
      <span>Sort</span>
      <select bind:value={sort} data-od-id="contact-sort">
        <option value="name">Name A–Z</option>
        <option value="updated">Recently updated</option>
        <option value="newest">Newest added</option>
        <option value="company">Company</option>
        <option value="birthday">Next birthday</option>
      </select>
    </label>
  </div>

  <div
    class="contact-result-line"
    role="status"
    aria-live="polite"
    data-od-id="contact-result-count"
  >
    <span>[ CONTACTS.SHOWN ]</span>
    <strong>{filteredContacts.length.toLocaleString("en-US")} shown</strong>
  </div>

  {#if availableTags.length || tagFilters.length}
    <nav
      class="tag-filter-bar"
      aria-label="Filter contacts by tag"
      data-od-id="contact-tag-filters"
    >
      <span>[ TAG.FILTER ]</span>
      <div class="tag-filter-list">
        {#each availableTags as tag, index (tag.key)}
          <button
            class:active={tagFilters.includes(tag.key)}
            type="button"
            aria-pressed={tagFilters.includes(tag.key)}
            onclick={() => toggleTagFilter(tag.key)}
            data-od-id={tagControlId(tag.key, index)}
          >
            {tag.label}<small>{tag.count}</small>
          </button>
        {/each}
      </div>
      {#if tagFilters.length}
        <button
          class="clear-tags"
          type="button"
          onclick={() => (tagFilters = [])}
          data-od-id="clear-contact-tag-filters">Clear</button
        >
      {/if}
    </nav>
  {/if}

  <div class="contacts-layout">
    <section
      class="contact-ledger"
      aria-label="Contacts"
      data-od-id="contact-ledger"
    >
      {#if loading}
        <div class="empty-state">Loading contacts…</div>
      {:else if filteredContacts.length}
        <AnimatedList
          items={filteredContacts}
          getKey={(contact) => contact.id}
          showGradients={false}
          enableArrowNavigation={false}
          displayScrollbar={true}
          class="contacts-animated-list"
        >
          {#snippet children(contact)}
            <button
              class="contact-row"
              type="button"
              onclick={() => openDetail(contact)}
              data-od-id={`contact-card-${contact.id}`}
            >
              {#if contact.has_photo}
                <img
                  class="avatar photo"
                  src={contactPhotoUrl(contact)}
                  alt=""
                />
              {:else}
                <span class="avatar" aria-hidden="true"
                  >{initials(contact)}</span
                >
              {/if}
              <span class="identity">
                <span
                  ><strong>{displayName(contact)}</strong
                  >{#if contact.nickname}<small>“{contact.nickname}”</small
                    >{/if}</span
                >
                <small
                  >{[contact.job_title, contact.company]
                    .filter(Boolean)
                    .join(" · ") || "No work details"}</small
                >
              </span>
              <span class="contact-preview">
                <small
                  >{contact.emails[0]?.value ||
                    contact.phones[0]?.value ||
                    "No contact method"}</small
                >
                <span
                  >{#each contact.tags.slice(0, 2) as tag (tag)}<i>{tag}</i
                    >{/each}</span
                >
              </span>
              <span class="source-mark">{sourceLabel(contact)}</span>
              {#if contact.favorite}<Star
                  class="favorite-mark"
                  size={15}
                  fill="currentColor"
                  aria-label="Favorite"
                />{/if}
            </button>
          {/snippet}
        </AnimatedList>
      {:else}
        <div class="empty-state">
          <UsersRound size={30} strokeWidth={1.4} />
          <h3>
            {query || tagFilters.length
              ? "No matching people"
              : filter === "archived"
                ? "No archived contacts"
                : "Your relationship index is empty"}
          </h3>
          <p>
            {query || tagFilters.length
              ? "Try a broader name, company, or tag."
              : "Add a contact or import a Monica JSON file to begin."}
          </p>
        </div>
      {/if}
    </section>

    <aside class="relationship-rail" data-od-id="relationship-pulse">
      <header>
        <span>[ NEXT.UP ]</span>
        <h3>Relationship pulse</h3>
      </header>
      {#each upcomingDates as item, index (`${item.contact.id}-${item.label}-${item.next.toISOString()}`)}
        <button
          type="button"
          onclick={() => openDetail(item.contact)}
          data-od-id={`upcoming-${item.contact.id}-${index}`}
        >
          <CalendarDays size={15} />
          <span
            ><strong>{item.label}</strong><small
              >{displayName(item.contact)}</small
            ></span
          >
          <time datetime={item.next.toISOString()}
            >{new Intl.DateTimeFormat("en", {
              month: "short",
              day: "numeric",
            }).format(item.next)}</time
          >
        </button>
      {:else}
        <p>No birthdays or important dates recorded yet.</p>
      {/each}
      <footer>
        <span
          >{contactsData.dav_sources.filter((source) => !source.last_error)
            .length} healthy sources</span
        ><button type="button" onclick={openDav}>Manage sync</button>
      </footer>
    </aside>
  </div>

  <dialog
    class="contact-dialog dossier-dialog"
    {@attach captureDetailDialog}
    onclick={(event) => event.target === detailDialog && detailDialog?.close()}
    data-od-id="contact-detail-modal"
  >
    {#if selected}
      <header class="dossier-header">
        <div class="dossier-identity">
          {#if selected.has_photo}
            <img
              class="avatar large photo"
              src={contactPhotoUrl(selected)}
              alt=""
            />
          {:else}
            <span class="avatar large" aria-hidden="true"
              >{initials(selected)}</span
            >
          {/if}
          <div>
            <span>[ CONTACT.DOSSIER / {sourceLabel(selected)} ]</span>
            <h2>{displayName(selected)}</h2>
            <p>
              {[selected.job_title, selected.company]
                .filter(Boolean)
                .join(" · ") || "Personal contact"}
            </p>
          </div>
        </div>
        <div class="dialog-actions">
          <button
            class="ui-button ui-button--ghost ui-button--icon"
            class:active={selected.favorite}
            type="button"
            aria-label={selected.favorite ? "Remove favorite" : "Add favorite"}
            onclick={() => toggleFavorite(selected!)}
            ><Star
              size={17}
              fill={selected.favorite ? "currentColor" : "none"}
            /></button
          >
          <button class="ui-button ui-button--secondary" type="button" onclick={() => openEdit(selected!)}
            ><Pencil size={16} /> Edit</button
          >
          <button class="ui-button ui-button--ghost ui-button--icon"
            type="button"
            aria-label="Close contact"
            onclick={() => detailDialog?.close()}><X size={18} /></button
          >
        </div>
      </header>
      <div class="dossier-grid">
        <section data-od-id="contact-channels">
          <h3>Contact channels</h3>
          <div class="detail-list">
            {#each selected.emails as method (`email-${method.label}-${method.value}`)}<a
                href={`mailto:${method.value}`}
                ><Mail size={15} /><span
                  ><small>{method.label}</small>{method.value}</span
                ></a
              >{/each}
            {#each selected.phones as method (`phone-${method.label}-${method.value}`)}<a
                href={`tel:${method.value}`}
                ><Phone size={15} /><span
                  ><small>{method.label}</small>{method.value}</span
                ></a
              >{/each}
            {#if selected.emails.length + selected.phones.length === 0}<p>
                No contact methods recorded.
              </p>{/if}
          </div>
        </section>
        <section data-od-id="contact-important-dates">
          <h3>Important dates</h3>
          <div class="date-list">
            {#if selected.birthday}<div>
                <CalendarDays size={15} /><span
                  ><small>Birthday</small>{formatDate(selected.birthday)}</span
                >
              </div>{/if}
            {#each selected.important_dates as date (`${date.label}-${date.date}`)}<div
              >
                <CalendarDays size={15} /><span
                  ><small>{date.label}</small>{formatDate(date.date)}</span
                >
              </div>{/each}
            {#if !selected.birthday && selected.important_dates.length === 0}<p>
                No important dates recorded.
              </p>{/if}
          </div>
        </section>
        <section class="wide" data-od-id="contact-relationship-context">
          <h3>Relationship context</h3>
          <p>
            {selected.relationship_context ||
              "No relationship context recorded."}
          </p>
        </section>
        <section data-od-id="contact-addresses">
          <h3>Addresses</h3>
          <div class="address-list">
            {#each selected.addresses as address (`${address.label}-${address.street}-${address.city}`)}<div
              >
                <MapPin size={15} /><span
                  ><small>{address.label}</small>{[
                    address.street,
                    address.city,
                    address.region,
                    address.postal_code,
                    address.country,
                  ]
                    .filter(Boolean)
                    .join(", ")}</span
                >
              </div>{:else}<p>No addresses recorded.</p>{/each}
          </div>
        </section>
        <section data-od-id="contact-profile-details">
          <h3>Profile</h3>
          <dl>
            <div>
              <dt>Organization</dt>
              <dd>{selected.company || "Not set"}</dd>
            </div>
            <div>
              <dt>Role</dt>
              <dd>{selected.job_title || "Not set"}</dd>
            </div>
          </dl>
        </section>
        <section class="wide notes-section" data-od-id="contact-private-notes">
          <h3>Private notes</h3>
          <p>{selected.notes || "No private notes recorded."}</p>
          {#if selected.tags.length}<div class="tag-list">
              {#each selected.tags as tag (tag)}<span>{tag}</span>{/each}
            </div>{/if}
        </section>
      </div>
      <footer class="danger-footer">
        <span
          >{selected.archived
            ? "This contact is archived."
            : "Deleting removes this contact from Pandan only."}</span
        ><button
          class="ui-button ui-button--danger"
          class:confirm={deleteId === selected.id}
          type="button"
          onclick={() => removeContact(selected!)}
          ><Trash2 size={15} />
          {deleteId === selected.id
            ? "Confirm delete"
            : "Delete contact"}</button
        >
      </footer>
    {/if}
  </dialog>

  <dialog
    class="contact-dialog editor-dialog"
    {@attach captureEditorDialog}
    onclose={clearContactPhotoDraft}
    onclick={(event) => event.target === editorDialog && editorDialog?.close()}
    data-od-id="contact-editor-modal"
  >
    <header>
      <div>
        <span>[ CONTACT.EDIT ]</span>
        <h2>{editingId ? "Edit contact" : "Add contact"}</h2>
      </div>
      <button class="ui-button ui-button--ghost ui-button--icon"
        type="button"
        aria-label="Close editor"
        onclick={() => editorDialog?.close()}><X size={18} /></button
      >
    </header>
    <form onsubmit={saveContact}>
      <div class="editor-scroll">
        <fieldset>
          <legend>Identity</legend>
          <div class="contact-photo-editor" data-od-id="contact-photo-editor">
            <span class="contact-photo-preview" aria-hidden="true">
              {#if editorPhotoSource()}
                <img src={editorPhotoSource()} alt="" />
              {:else}
                {draftInitials()}
              {/if}
            </span>
            <div>
              <strong>Profile picture</strong>
              <small>JPEG, PNG, WebP, or AVIF up to 10 MB.</small>
            </div>
            <div class="contact-photo-actions">
              <label class="ui-button ui-button--secondary">
                <ImagePlus size={15} /> Choose image
                <input
                  class="sr-only"
                  type="file"
                  accept="image/jpeg,image/png,image/webp,image/avif"
                  onchange={selectContactPhoto}
                  data-od-id="choose-contact-photo"
                />
              </label>
              <button
                class="ui-button ui-button--danger"
                type="button"
                disabled={contactPhotoReset ||
                  (!contactPhotoFile && !editingContact()?.has_photo)}
                onclick={removeContactPhotoDraft}
                data-od-id="remove-contact-photo">Remove</button
              >
            </div>
          </div>
          <div class="form-grid three">
            <label
              >First name<input
                bind:value={contactDraft.first_name}
                maxlength="120"
                {@attach captureEditorName}
              /></label
            ><label
              >Middle name<input
                bind:value={contactDraft.middle_name}
                maxlength="120"
              /></label
            ><label
              >Last name<input
                bind:value={contactDraft.last_name}
                maxlength="120"
              /></label
            >
          </div>
          <div class="form-grid two">
            <label
              >Nickname<input
                bind:value={contactDraft.nickname}
                maxlength="120"
              /></label
            ><div class="birthday-editor">
              <label>
                Birthday
                {#if birthdayYearUnknown}
                  <input
                    bind:value={birthdayMonthDay}
                    inputmode="numeric"
                    maxlength="5"
                    pattern="[0-1][0-9]-[0-3][0-9]"
                    placeholder="MM-DD"
                    aria-label="Birthday month and day"
                  />
                {:else}
                  <input type="date" bind:value={contactDraft.birthday} />
                {/if}
              </label>
              <button
                class="toggle-pill birthday-toggle"
                class:enabled={birthdayYearUnknown}
                type="button"
                role="switch"
                aria-checked={birthdayYearUnknown}
                onclick={toggleBirthdayYearUnknown}
                data-od-id="birthday-year-toggle"
              >
                <span class="toggle-track" aria-hidden="true"></span>
                <span>Year unknown</span>
              </button>
            </div>
          </div>
        </fieldset>
        <fieldset>
          <legend>Work</legend>
          <div class="form-grid two">
            <label
              >Organization<input
                bind:value={contactDraft.company}
                maxlength="160"
              /></label
            ><label
              >Role<input
                bind:value={contactDraft.job_title}
                maxlength="160"
              /></label
            >
          </div>
        </fieldset>
        <fieldset>
          <legend>Contact channels</legend>
          <div class="repeat-grid">
            {#each contactDraft.emails as email (email)}<div class="repeat-row">
                <select bind:value={email.label} aria-label="Email label"
                  ><option>personal</option><option>work</option><option
                    >other</option
                  ></select
                ><input
                  type="email"
                  bind:value={email.value}
                  placeholder="Email address"
                  aria-label="Email address"
                /><button
                  type="button"
                  aria-label="Remove email"
                  onclick={() =>
                    (contactDraft.emails = contactDraft.emails.filter(
                      (item) => item !== email,
                    ))}><X size={15} /></button
                >
              </div>{/each}
            <button
              class="add-row"
              type="button"
              onclick={() =>
                contactDraft.emails.push({ label: "personal", value: "" })}
              ><Plus size={14} /> Add email</button
            >
            {#each contactDraft.phones as phone (phone)}<div class="repeat-row">
                <select bind:value={phone.label} aria-label="Phone label"
                  ><option>mobile</option><option>home</option><option
                    >work</option
                  ><option>other</option></select
                ><input
                  type="tel"
                  bind:value={phone.value}
                  placeholder="Phone number"
                  aria-label="Phone number"
                /><button
                  type="button"
                  aria-label="Remove phone"
                  onclick={() =>
                    (contactDraft.phones = contactDraft.phones.filter(
                      (item) => item !== phone,
                    ))}><X size={15} /></button
                >
              </div>{/each}
            <button
              class="add-row"
              type="button"
              onclick={() =>
                contactDraft.phones.push({ label: "mobile", value: "" })}
              ><Plus size={14} /> Add phone</button
            >
          </div>
        </fieldset>
        <fieldset>
          <legend>Addresses</legend
          >{#each contactDraft.addresses as address (address)}<div
              class="nested-card"
            >
              <div class="form-grid three">
                <label
                  >Label<input
                    bind:value={address.label}
                    maxlength="40"
                  /></label
                ><label class="span-two"
                  >Street<input
                    bind:value={address.street}
                    maxlength="240"
                  /></label
                >
              </div>
              <div class="form-grid three">
                <label
                  >City<input
                    bind:value={address.city}
                    maxlength="120"
                  /></label
                ><label
                  >Region<input
                    bind:value={address.region}
                    maxlength="120"
                  /></label
                ><label
                  >Postal code<input
                    bind:value={address.postal_code}
                    maxlength="40"
                  /></label
                >
              </div>
              <div class="nested-footer">
                <label
                  >Country<input
                    bind:value={address.country}
                    maxlength="120"
                  /></label
                ><button
                  type="button"
                  onclick={() =>
                    (contactDraft.addresses = contactDraft.addresses.filter(
                      (item) => item !== address,
                    ))}>Remove</button
                >
              </div>
            </div>{/each}<button
            class="add-row"
            type="button"
            onclick={() =>
              contactDraft.addresses.push({
                label: "home",
                street: "",
                city: "",
                region: "",
                postal_code: "",
                country: "",
              })}><Plus size={14} /> Add address</button
          >
        </fieldset>
        <fieldset>
          <legend>Important dates</legend
          >{#each contactDraft.important_dates as date (date)}<div
              class="repeat-row date-row"
            >
              <input
                bind:value={date.label}
                maxlength="80"
                placeholder="Label"
                aria-label="Date label"
              /><input
                type="date"
                bind:value={date.date}
                aria-label="Important date"
              /><button
                class="toggle-pill date-toggle"
                class:enabled={date.recurring}
                type="button"
                role="switch"
                aria-checked={date.recurring}
                onclick={() => (date.recurring = !date.recurring)}
              >
                <span class="toggle-track" aria-hidden="true"></span>
                <span>Annual</span>
              </button><button
                type="button"
                aria-label="Remove date"
                onclick={() =>
                  (contactDraft.important_dates =
                    contactDraft.important_dates.filter(
                      (item) => item !== date,
                    ))}><X size={15} /></button
              >
            </div>{/each}<button
            class="add-row"
            type="button"
            onclick={() =>
              contactDraft.important_dates.push({
                label: "",
                date: "",
                recurring: true,
              })}><Plus size={14} /> Add important date</button
          >
        </fieldset>
        <fieldset>
          <legend>Context</legend>
          <div class="tag-field">
            <span class="field-label">Tags</span>
            <div class="tag-editor">
              {#if contactDraft.tags.length}
                <div class="draft-tag-list" aria-label="Contact tags">
                  {#each contactDraft.tags as tag (tag)}
                    <button
                      class="draft-tag"
                      type="button"
                      aria-label={`Remove ${tag} tag`}
                      onclick={() => removeDraftTag(tag)}
                    >
                      <span>{tag}</span><X size={13} />
                    </button>
                  {/each}
                </div>
              {/if}
              <div class="tag-entry">
                <input
                  bind:value={tagText}
                  maxlength="1200"
                  placeholder="Type a tag, then press Enter"
                  aria-label="Add tag"
                  onkeydown={handleTagKeydown}
                />
                <button
                  type="button"
                  disabled={!tagText.trim()}
                  onclick={commitDraftTags}
                  data-od-id="add-contact-tag"
                >
                  <Plus size={14} /> Add tag
                </button>
              </div>
            </div>
            <small>Enter or comma adds a tag. Backspace removes the last.</small
            >
          </div>
          <label
            >Relationship context<textarea
              bind:value={contactDraft.relationship_context}
              maxlength="4000"
              rows="4"
              placeholder="How you know each other, people connected to them, or useful context"
            ></textarea></label
          ><label
            >Private notes<textarea
              bind:value={contactDraft.notes}
              maxlength="20000"
              rows="6"
              placeholder="Details worth remembering"></textarea></label
          >
          <div class="toggle-row">
          <button
            class="toggle-pill setting-toggle"
              class:enabled={contactDraft.favorite}
              type="button"
              role="switch"
              aria-checked={contactDraft.favorite}
              onclick={() => (contactDraft.favorite = !contactDraft.favorite)}
              data-od-id="contact-favorite-toggle"
            >
              <span class="toggle-track" aria-hidden="true"></span>
              <span class="toggle-copy">
                <strong>Favorite</strong>
                <small>Keep this person close at hand.</small>
              </span>
            </button>
            <button
              class="toggle-pill setting-toggle destructive-toggle"
              class:enabled={contactDraft.archived}
              type="button"
              role="switch"
              aria-checked={contactDraft.archived}
              onclick={() => (contactDraft.archived = !contactDraft.archived)}
              data-od-id="contact-archived-toggle"
            >
              <span class="toggle-track" aria-hidden="true"></span>
              <span class="toggle-copy">
                <strong>Archived</strong>
                <small>Move this person out of the active directory.</small>
              </span>
            </button>
          </div>
        </fieldset>
        {#if formError}<p class="form-error" role="alert">{formError}</p>{/if}
      </div>
      <footer>
        <button class="ui-button ui-button--secondary" type="button" onclick={() => editorDialog?.close()}
          >Cancel</button
        ><button class="ui-button ui-button--primary" type="submit" disabled={saving}
          >{saving
            ? "Saving…"
            : editingId
              ? "Save changes"
              : "Add contact"}</button
        >
      </footer>
    </form>
  </dialog>

  <dialog
    class="contact-dialog dav-dialog"
    {@attach captureDavDialog}
    onclick={(event) => event.target === davDialog && davDialog?.close()}
    data-od-id="carddav-modal"
  >
    <header>
      <div>
        <span>[ CONTACTS.CARDDAV ]</span>
        <h2>Connected address books</h2>
      </div>
      <button class="ui-button ui-button--ghost ui-button--icon"
        type="button"
        aria-label="Close DAV settings"
        onclick={() => davDialog?.close()}><X size={18} /></button
      >
    </header>
    <div class="dav-content">
      <section>
        <h3>Resources</h3>
        {#each contactsData.dav_sources as source (source.id)}<article
            class="source-card"
            data-od-id={`dav-source-${source.id}`}
          >
            <Server size={17} />
            <div>
              <strong>{source.name}</strong><small>{source.url}</small><span
                class={source.last_error ? "error" : ""}
                >{source.last_error ||
                  formatTimestamp(source.last_synced_at)}</span
              >
            </div>
            <button
              type="button"
              aria-label={`Sync ${source.name}`}
              disabled={Boolean(busyId)}
              onclick={() => syncDavSource(source)}
              ><RefreshCw
                class={busyId === source.id ? "spinning" : ""}
                size={15}
              /></button
            ><button
              class="ui-button ui-button--danger ui-button--icon"
              class:confirm={sourceDeleteId === source.id}
              type="button"
              aria-label={`Remove ${source.name}`}
              disabled={Boolean(busyId)}
              onclick={() => removeDavSource(source)}
              ><Trash2 size={15} /></button
            >
          </article>{:else}<p class="muted-copy">
            No CardDAV resources connected.
          </p>{/each}
      </section>
      <form onsubmit={addDavSource}>
        <h3>Add resource</h3>
        <p>
          Use the direct HTTPS URL for a CardDAV address book. Sync currently
          pulls vCards into Pandan.
        </p>
        <label
          >Name<input
            bind:value={davName}
            maxlength="80"
            placeholder="Personal address book"
            required
          /></label
        ><label
          >Address-book URL<input
            type="url"
            bind:value={davUrl}
            maxlength="2048"
            placeholder="https://dav.example.com/addressbooks/me/contacts/"
            required
          /></label
        >
        <div class="form-grid two">
          <label
            >Username<input
              bind:value={davUsername}
              maxlength="320"
              autocomplete="username"
            /></label
          ><label
            >Password<input
              type="password"
              bind:value={davPassword}
              maxlength="4096"
              autocomplete="current-password"
              disabled={!contactsData.secret_storage_enabled}
            /></label
          >
        </div>
        {#if !contactsData.secret_storage_enabled}<p class="muted-copy">
            Password storage is unavailable until PANDAN_WIDGET_SECRET_KEY is
            configured. Anonymous DAV resources can still be added.
          </p>{/if}{#if davError}<p class="form-error" role="alert">
            {davError}
          </p>{/if}<button class="ui-button ui-button--primary" type="submit" disabled={saving}
          >{saving ? "Connecting…" : "Add resource"}</button
        >
      </form>
    </div>
  </dialog>
</section>

<style>
  .contacts-page {
    display: flex;
    height: calc(100dvh - 76px);
    min-width: 0;
    flex-direction: column;
    gap: 18px;
    overflow: hidden;
    padding: clamp(24px, 3vw, 42px);
  }
  .contacts-header {
    display: flex;
    align-items: end;
    justify-content: space-between;
    gap: 24px;
    border-bottom: 1px solid var(--border);
    padding-bottom: 18px;
  }
  .contact-dialog header span,
  .relationship-rail header span {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.09em;
  }
  .contacts-header h2 {
    margin-top: 8px;
    font-family: var(--font-mono);
    font-size: clamp(26px, 3vw, 42px);
    font-weight: 540;
    letter-spacing: -0.04em;
  }
  .contacts-header p {
    margin-top: 7px;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 11px;
  }
  .header-actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 7px;
  }
  button,
  input,
  select,
  textarea {
    font: inherit;
  }
  button {
    color: inherit;
  }
  .header-actions button:not(.ui-button),
  .dialog-actions button:not(.ui-button),
  .filter-group button:not(.ui-button),
  .contact-dialog footer button:not(.ui-button),
  .add-row {
    display: inline-flex;
    min-height: 44px;
    align-items: center;
    justify-content: center;
    gap: 7px;
    border: 1px solid var(--border);
    background: transparent;
    padding: 0 13px;
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.02em;
  }
  button:not(.ui-button):hover {
    border-color: var(--fg);
  }
  .page-error,
  .form-error {
    margin: 0;
    border: 1px solid oklch(60% 0.16 25 / 0.5);
    background: oklch(20% 0.04 25 / 0.75);
    padding: 10px 12px;
    color: oklch(82% 0.09 25);
    font-family: var(--font-mono);
    font-size: 11px;
  }
  .import-status {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    border: 1px solid var(--border);
    background: color-mix(in oklch, var(--page-surface, var(--surface)) 90%, transparent);
    padding: 10px 12px;
    font-family: var(--font-mono);
    font-size: 11px;
  }
  .contact-metrics {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    border: 1px solid var(--border);
    background: color-mix(in oklch, var(--page-surface, var(--surface)) 88%, transparent);
  }
  .contact-metrics div {
    display: flex;
    min-height: 74px;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    border-right: 1px solid var(--border);
    padding: 12px 16px;
  }
  .contact-metrics div:last-child {
    border-right: 0;
  }
  .contact-metrics span {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 9px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  .contact-metrics strong {
    font-family: var(--font-mono);
    font-size: 22px;
    font-weight: 520;
  }
  .contacts-toolbar {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .search-field {
    display: flex;
    min-height: 44px;
    flex: 1;
    align-items: center;
    gap: 9px;
    border: 1px solid var(--border);
    background: color-mix(in oklch, var(--page-surface, var(--surface)) 90%, transparent);
    padding: 0 12px;
  }
  .search-field input {
    min-width: 0;
    flex: 1;
    border: 0;
    outline: 0;
    background: transparent;
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 11px;
  }
  .filter-group {
    display: flex;
    border: 1px solid var(--border);
    background: color-mix(in oklch, var(--page-surface, var(--surface)) 82%, var(--fg-soft));
  }
  .filter-group button {
    min-height: 42px;
    border: 0;
    border-right: 1px solid var(--border);
    background: color-mix(in oklch, var(--page-surface, var(--surface)) 90%, var(--fg-soft));
  }
  .filter-group button:last-child {
    border-right: 0;
  }
  .filter-group button:hover {
    background: color-mix(in oklch, var(--fg) 9%, var(--surface));
  }
  .filter-group button.active {
    background: color-mix(in oklch, var(--fg) 13%, var(--surface));
    box-shadow: inset 0 -2px 0 var(--fg);
  }
  .sort-field {
    display: grid;
    grid-template-columns: auto minmax(132px, 1fr);
    min-height: 44px;
    align-items: center;
    border: 1px solid var(--border);
    background: color-mix(in oklch, var(--page-surface, var(--surface)) 90%, transparent);
  }
  .sort-field > span {
    padding: 0 9px 0 11px;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 9px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }
  .sort-field select {
    height: 42px;
    border: 0;
    border-left: 1px solid var(--border);
    outline: 0;
    background: var(--bg);
    color: var(--fg);
    padding: 0 28px 0 10px;
    font-family: var(--font-mono);
    font-size: 10px;
  }
  .contact-result-line {
    position: relative;
    z-index: 1;
    display: flex;
    min-height: 38px;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    border: 1px solid var(--border);
    background: color-mix(in oklch, var(--page-surface, var(--surface)) 94%, var(--fg-soft));
    padding: 8px 12px;
    box-shadow: 0 9px 24px
      color-mix(in oklch, var(--fg) 10%, transparent);
  }
  .contact-result-line > span,
  .contact-result-line > strong {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.06em;
    white-space: nowrap;
  }
  .contact-result-line > strong {
    min-width: 12ch;
    color: var(--fg);
    font-variant-numeric: tabular-nums;
    font-weight: 550;
    text-align: right;
  }
  .tag-filter-bar {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 12px;
    border: 1px solid var(--border);
    background: color-mix(in oklch, var(--page-surface, var(--surface)) 90%, transparent);
    padding: 8px 10px;
  }
  .tag-filter-bar > span {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 9px;
    letter-spacing: 0.08em;
    white-space: nowrap;
  }
  .tag-filter-list {
    display: flex;
    min-width: 0;
    gap: 6px;
    overflow-x: auto;
    scrollbar-width: thin;
  }
  .tag-filter-list button,
  .clear-tags {
    display: inline-flex;
    min-height: 44px;
    flex: 0 0 auto;
    align-items: center;
    justify-content: center;
    gap: 7px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: transparent;
    padding: 0 12px;
    font-family: var(--font-mono);
    font-size: 9px;
    white-space: nowrap;
  }
  .tag-filter-list button small {
    color: var(--muted);
    font-size: 8px;
  }
  .tag-filter-list button.active {
    border-color: var(--fg);
    background: var(--fg);
    color: var(--bg);
  }
  .tag-filter-list button.active small {
    color: inherit;
  }
  .clear-tags {
    border-color: transparent;
    color: var(--muted);
    text-decoration: underline;
  }

  .contacts-layout {
    display: grid;
    min-height: 0;
    flex: 1;
    grid-template-columns: minmax(0, 1fr) minmax(230px, 280px);
    gap: 16px;
  }
  .contact-ledger,
  .relationship-rail {
    border: 1px solid var(--border);
    background: color-mix(in oklch, var(--page-surface, var(--surface)) 92%, transparent);
  }
  .contact-ledger {
    min-height: 0;
    overflow: hidden;
  }
  :global(.contacts-animated-list) {
    height: 100%;
    min-height: 0;
    overflow-y: auto;
    overscroll-behavior: contain;
    scrollbar-gutter: stable;
  }
  .contact-row {
    position: relative;
    display: grid;
    width: 100%;
    min-height: 78px;
    grid-template-columns:
      44px minmax(170px, 1.1fr) minmax(180px, 1fr)
      auto 18px;
    align-items: center;
    gap: 13px;
    border: 0;
    border-bottom: 1px solid var(--border);
    background: transparent;
    padding: 13px 16px;
    text-align: left;
  }
  :global(.animated-list-item:last-child) .contact-row {
    border-bottom: 0;
  }
  .contact-row:hover {
    background: var(--fg-soft);
  }
  .avatar {
    display: grid;
    width: 40px;
    height: 40px;
    flex: 0 0 auto;
    place-items: center;
    border: 1px solid var(--border);
    background: var(--fg-soft);
    font-family: var(--font-mono);
    font-size: 11px;
    letter-spacing: 0.04em;
  }
  .avatar.photo {
    display: block;
    object-fit: cover;
    object-position: center;
  }

  .avatar.large {
    width: 62px;
    height: 62px;
    font-size: 16px;
  }
  .identity,
  .identity > span,
  .contact-preview {
    display: flex;
    min-width: 0;
  }
  .identity {
    flex-direction: column;
    gap: 5px;
  }
  .identity > span {
    align-items: baseline;
    gap: 8px;
  }
  .identity strong {
    overflow: hidden;
    text-overflow: ellipsis;
    font-size: 13px;
    font-weight: 550;
  }
  .identity small,
  .contact-preview small {
    overflow: hidden;
    color: var(--muted);
    font-size: 10px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .contact-preview {
    flex-direction: column;
    gap: 7px;
  }
  .contact-preview > span,
  .tag-list {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }
  .contact-preview i,
  .tag-list span {
    border: 1px solid var(--border);
    border-radius: 999px;
    padding: 2px 7px;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 8px;
    font-style: normal;
  }
  .source-mark {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 9px;
  }
  :global(.favorite-mark) {
    color: var(--fg);
  }
  .empty-state {
    display: grid;
    min-height: 360px;
    place-content: center;
    justify-items: center;
    padding: 36px;
    color: var(--muted);
    text-align: center;
  }
  .empty-state h3 {
    margin: 15px 0 5px;
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 15px;
    font-weight: 550;
  }
  .empty-state p {
    max-width: 48ch;
    font-size: 12px;
  }
  .relationship-rail {
    max-height: 100%;
    align-self: start;
    overflow: hidden;
  }
  .relationship-rail > header {
    border-bottom: 1px solid var(--border);
    padding: 16px;
  }
  .relationship-rail h3 {
    margin-top: 6px;
    font-family: var(--font-mono);
    font-size: 14px;
    font-weight: 540;
  }
  .relationship-rail > button {
    display: grid;
    width: 100%;
    min-height: 66px;
    grid-template-columns: 18px 1fr auto;
    align-items: center;
    gap: 9px;
    border: 0;
    border-bottom: 1px solid var(--border);
    background: transparent;
    padding: 11px 14px;
    text-align: left;
  }
  .relationship-rail > button:hover {
    background: var(--fg-soft);
  }
  .relationship-rail button span {
    display: flex;
    min-width: 0;
    flex-direction: column;
  }
  .relationship-rail button strong {
    overflow: hidden;
    font-size: 11px;
    font-weight: 550;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .relationship-rail button small,
  .relationship-rail time {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 9px;
  }
  .relationship-rail > p {
    padding: 22px 16px;
    color: var(--muted);
    font-size: 11px;
    line-height: 1.6;
  }
  .relationship-rail > footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 13px 14px;
  }
  .relationship-rail footer span,
  .relationship-rail footer button {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 9px;
  }
  .relationship-rail footer button {
    min-height: 36px;
    border: 0;
    background: transparent;
    text-decoration: underline;
  }
  .contact-dialog {
    position: fixed;
    inset: 0;
    width: min(980px, calc(100vw - 32px));
    max-height: min(90vh, 920px);
    margin: auto;
    border: 1px solid var(--border);
    background: var(--page-surface, var(--surface));
    color: var(--fg);
    padding: 0;
    box-shadow: var(--shadow);
  }
  .contact-dialog::backdrop {
    background: oklch(5% 0 0 / 0.72);
    backdrop-filter: blur(5px);
  }
  .contact-dialog > header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    border-bottom: 1px solid var(--border);
    padding: 18px 20px;
  }
  .contact-dialog > header h2 {
    margin-top: 5px;
    font-family: var(--font-mono);
    font-size: 20px;
    font-weight: 550;
    letter-spacing: -0.01em;
  }
  .dossier-header {
    position: sticky;
    top: 0;
    z-index: 2;
    background: var(--page-surface, var(--surface));
  }
  .dossier-identity {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 15px;
  }
  .dossier-identity h2 {
    font-size: clamp(21px, 3vw, 30px) !important;
  }
  .dossier-identity p {
    margin-top: 5px;
    color: var(--muted);
    font-size: 11px;
  }
  .dialog-actions {
    display: flex;
    gap: 7px;
  }
  .dialog-actions button.active {
    color: var(--accent);
  }
  .dossier-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
  }
  .dossier-grid > section {
    min-width: 0;
    border-right: 1px solid var(--border);
    border-bottom: 1px solid var(--border);
    padding: 22px;
  }
  .dossier-grid > section:nth-child(even),
  .dossier-grid > section.wide {
    border-right: 0;
  }
  .dossier-grid > section.wide {
    grid-column: 1 / -1;
  }
  .dossier-grid h3,
  .dav-content h3 {
    margin-bottom: 14px;
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 550;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }
  .dossier-grid section > p {
    color: var(--muted);
    font-size: 12px;
    line-height: 1.7;
    white-space: pre-wrap;
  }
  .detail-list,
  .date-list,
  .address-list {
    display: grid;
    gap: 8px;
  }
  .detail-list a,
  .date-list > div,
  .address-list > div {
    display: flex;
    min-height: 44px;
    align-items: center;
    gap: 10px;
    border: 1px solid var(--border);
    padding: 9px 11px;
    color: var(--fg);
    font-size: 11px;
    text-decoration: none;
  }
  .detail-list a:hover {
    border-color: var(--fg);
  }
  .detail-list span,
  .date-list span,
  .address-list span {
    display: flex;
    min-width: 0;
    flex-direction: column;
    overflow-wrap: anywhere;
  }
  .detail-list small,
  .date-list small,
  .address-list small {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 8px;
    text-transform: uppercase;
  }
  .detail-list p,
  .date-list p,
  .address-list p {
    color: var(--muted);
    font-size: 11px;
  }
  .dossier-grid dl {
    display: grid;
    gap: 8px;
    margin: 0;
  }
  .dossier-grid dl div {
    display: flex;
    justify-content: space-between;
    gap: 16px;
    border-bottom: 1px solid var(--border);
    padding-bottom: 8px;
  }
  .dossier-grid dt {
    color: var(--muted);
    font-size: 10px;
  }
  .dossier-grid dd {
    margin: 0;
    font-size: 11px;
    text-align: right;
  }
  .notes-section .tag-list {
    margin-top: 16px;
  }
  .danger-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 14px 20px;
  }
  .danger-footer span {
    color: var(--muted);
    font-size: 10px;
  }
  button.confirm {
    border-color: oklch(62% 0.19 25) !important;
    color: oklch(75% 0.14 25) !important;
  }
  .editor-dialog,
  .dav-dialog {
    overflow: auto;
  }
  .editor-dialog {
    width: min(880px, calc(100vw - 32px));
    overflow: hidden;
  }
  .editor-dialog[open] {
    display: flex;
    flex-direction: column;
  }
  .editor-dialog > header {
    position: sticky;
    top: 0;
    z-index: 3;
    background: var(--page-surface, var(--surface));
  }
  .editor-dialog form {
    min-height: 0;
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .editor-scroll {
    min-height: 0;
    flex: 1;
    display: grid;
    overflow-y: auto;
    overscroll-behavior: contain;
    scrollbar-gutter: stable;
  }
  .editor-dialog fieldset {
    display: grid;
    gap: 18px;
    margin: 0;
    border: 0;
    border-bottom: 1px solid var(--border);
    padding: 24px clamp(18px, 3vw, 28px) 30px;
  }
  .editor-dialog legend {
    margin-bottom: 2px;
    padding: 10px 0 0;
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 550;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }
  .contact-photo-editor {
    display: grid;
    grid-template-columns: 68px minmax(0, 1fr) auto;
    align-items: center;
    gap: 14px;
    border: 1px solid var(--border);
    background: color-mix(in oklch, var(--page-surface, var(--surface)) 88%, transparent);
    padding: 12px;
  }
  .contact-photo-preview {
    display: grid;
    width: 68px;
    height: 68px;
    place-items: center;
    overflow: hidden;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 18px;
  }
  .contact-photo-preview img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .contact-photo-editor > div:nth-child(2) {
    display: grid;
    gap: 4px;
  }
  .contact-photo-editor strong {
    color: var(--fg);
    font-size: 12px;
    font-weight: 550;
  }
  .contact-photo-editor small {
    color: var(--muted);
    font-size: 9px;
    line-height: 1.5;
  }
  .contact-photo-actions {
    display: flex;
    gap: 8px;
  }
  .contact-photo-actions .ui-button {
    min-height: 44px;
    white-space: nowrap;
  }
  .contact-photo-actions label {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    color: var(--fg);
    cursor: pointer;
  }
  .contact-photo-actions button:disabled {
    cursor: not-allowed;
    opacity: 0.45;
  }
  .form-grid {
    display: grid;
    gap: 14px;
  }
  .form-grid.three {
    grid-template-columns: repeat(3, 1fr);
  }
  .form-grid.two {
    grid-template-columns: repeat(2, 1fr);
  }
  .birthday-editor {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: end;
    gap: 8px;
  }
  .birthday-toggle {
    min-height: 42px;
    white-space: nowrap;
  }
  .span-two {
    grid-column: span 2;
  }
  .editor-dialog label,
  .dav-dialog label {
    display: grid;
    gap: 8px;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 9px;
    letter-spacing: 0.03em;
  }
  .editor-dialog input,
  .editor-dialog select,
  .editor-dialog textarea,
  .dav-dialog input {
    width: 100%;
    min-height: 44px;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--fg);
    padding: 10px 11px;
  }
  .editor-dialog textarea {
    min-height: 96px;
    resize: vertical;
    line-height: 1.55;
  }
  .repeat-grid {
    display: grid;
    gap: 10px;
  }
  .repeat-row {
    display: grid;
    grid-template-columns: 120px 1fr 44px;
    gap: 8px;
  }
  .repeat-row > button {
    display: grid;
    min-height: 44px;
    place-items: center;
    border: 1px solid var(--border);
    background: transparent;
  }
  .date-row {
    grid-template-columns: 1fr 180px 116px 44px;
  }
  .add-row {
    margin-top: 2px;
    justify-self: start;
  }
  .nested-card {
    display: grid;
    gap: 14px;
    border: 1px solid var(--border);
    padding: 15px;
  }
  .nested-footer {
    display: grid;
    grid-template-columns: 1fr auto;
    align-items: end;
    gap: 10px;
  }
  .nested-footer button {
    min-height: 44px;
    border: 1px solid var(--border);
    background: transparent;
    padding: 0 12px;
    font-size: 10px;
  }
  .tag-field {
    display: grid;
    gap: 8px;
  }
  .field-label,
  .tag-field > small {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 9px;
    letter-spacing: 0.03em;
  }
  .tag-field > small {
    line-height: 1.5;
  }
  .tag-editor {
    display: grid;
    gap: 9px;
    border: 1px solid var(--border);
    background: color-mix(in oklch, var(--bg) 88%, var(--surface));
    padding: 9px;
  }
  .draft-tag-list {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .draft-tag {
    display: inline-flex;
    min-height: 44px;
    align-items: center;
    gap: 7px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--page-surface, var(--surface));
    padding: 0 11px;
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 9px;
  }
  .draft-tag:hover {
    border-color: var(--fg);
    background: var(--fg-soft);
  }
  .tag-entry {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 8px;
  }
  .tag-entry button {
    display: inline-flex;
    min-height: 44px;
    align-items: center;
    justify-content: center;
    gap: 7px;
    border: 1px solid var(--border);
    background: transparent;
    padding: 0 13px;
    font-family: var(--font-mono);
    font-size: 9px;
  }
  .tag-entry button:disabled {
    opacity: 0.45;
  }
  .toggle-row {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
    margin-top: 2px;
  }
  .toggle-pill {
    display: flex !important;
    min-height: 44px;
    align-items: center;
    justify-content: flex-start;
    gap: 10px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--bg);
    padding: 7px 11px;
    color: var(--muted);
    text-align: left;
    transition:
      border-color 150ms var(--ease-out),
      background-color 150ms var(--ease-out),
      color 150ms var(--ease-out);
  }
  .toggle-pill:hover,
  .toggle-pill.enabled {
    border-color: var(--fg);
    background: var(--fg-soft);
    color: var(--fg);
  }
  .toggle-track {
    position: relative;
    width: 34px;
    height: 20px;
    flex: 0 0 auto;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--page-surface, var(--surface));
  }
  .toggle-track::after {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--muted);
    content: "";
    transition:
      transform 150ms var(--ease-out),
      background-color 150ms var(--ease-out);
  }
  .toggle-pill.enabled .toggle-track {
    border-color: var(--fg);
    background: var(--fg);
  }
  .toggle-pill.enabled .toggle-track::after {
    background: var(--bg);
    transform: translateX(14px);
  }
  .destructive-toggle.destructive-toggle.enabled {
    border-color: var(--danger);
    background: color-mix(in oklch, var(--danger) 10%, transparent);
    color: var(--danger);
  }
  .destructive-toggle.destructive-toggle.enabled .toggle-track {
    border-color: var(--danger);
    background: var(--danger);
  }
  .toggle-copy {
    display: flex;
    min-width: 0;
    flex-direction: column;
    gap: 2px;
  }
  .toggle-copy strong {
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 550;
  }
  .toggle-copy small {
    color: var(--muted);
    font-size: 8px;
    line-height: 1.4;
  }
  .date-toggle {
    padding-inline: 10px;
    font-family: var(--font-mono);
    font-size: 9px;
  }
  .editor-dialog form > footer {
    flex: 0 0 auto;
    z-index: 3;
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    border-top: 1px solid var(--border);
    background: var(--page-surface, var(--surface));
    padding: 16px clamp(18px, 3vw, 28px);
  }
  .editor-dialog .form-error {
    margin: 16px clamp(18px, 3vw, 28px) 0;
  }
  .dav-content {
    display: grid;
    grid-template-columns: 1fr 1fr;
  }
  .dav-content > section,
  .dav-content > form {
    display: grid;
    align-content: start;
    gap: 10px;
    padding: 20px;
  }
  .dav-content > section {
    border-right: 1px solid var(--border);
  }
  .source-card {
    display: grid;
    grid-template-columns: 20px 1fr 40px 40px;
    align-items: center;
    gap: 9px;
    border: 1px solid var(--border);
    padding: 10px;
  }
  .source-card > div {
    display: flex;
    min-width: 0;
    flex-direction: column;
    gap: 3px;
  }
  .source-card strong {
    font-size: 11px;
  }
  .source-card small {
    overflow: hidden;
    color: var(--muted);
    font-size: 9px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .source-card span {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 8px;
  }
  .source-card span.error {
    color: oklch(75% 0.14 25);
  }
  .source-card button {
    display: grid;
    width: 40px;
    height: 40px;
    place-items: center;
    border: 1px solid var(--border);
    background: transparent;
  }
  .dav-content form > p {
    color: var(--muted);
    font-size: 11px;
    line-height: 1.6;
  }
  .dav-content form .ui-button--primary {
    justify-self: start;
  }
  .muted-copy {
    color: var(--muted);
    font-size: 10px !important;
    line-height: 1.6;
  }
  :global(.spinning) {
    animation: spin 0.8s linear infinite;
  }
  :is(input, select, textarea, button:not(.ui-button)):focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  @media (prefers-reduced-motion: no-preference) {
    .contact-dialog[open] {
      animation: dialog-enter 220ms var(--ease-out);
    }
    @keyframes dialog-enter {
      from {
        opacity: 0;
        transform: translateY(-18px);
      }
    }
  }
  @media (prefers-reduced-motion: reduce) {
    :global(.spinning) {
      animation: none;
    }
  }
  @media (max-width: 980px) {
    .contacts-header {
      align-items: stretch;
      flex-direction: column;
    }
    .header-actions {
      justify-content: flex-start;
    }
    .contacts-layout {
      grid-template-columns: 1fr;
    }
    .relationship-rail {
      display: none;
    }
    .contact-row {
      grid-template-columns: 44px minmax(0, 1fr) auto 18px;
    }
    .contact-preview {
      display: none;
    }
    .dav-content {
      grid-template-columns: 1fr;
    }
    .dav-content > section {
      border-right: 0;
      border-bottom: 1px solid var(--border);
    }
  }
  @media (max-width: 720px) {
    .sort-field {
      width: 100%;
    }

    .contact-photo-editor {
      grid-template-columns: 56px minmax(0, 1fr);
    }

    .contact-photo-preview {
      width: 56px;
      height: 56px;
    }

    .contact-photo-actions {
      grid-column: 1 / -1;
    }

    .contact-photo-actions .ui-button {
      flex: 1;
      justify-content: center;
    }

    .contacts-page {
      padding: 18px 14px;
      height: auto;
      min-height: calc(100dvh - 76px);
      overflow: visible;
    }
    .contact-metrics {
      grid-template-columns: 1fr 1fr;
    }
    .contact-metrics div:nth-child(2) {
      border-right: 0;
    }
    .contact-metrics div:nth-child(-n + 2) {
      border-bottom: 1px solid var(--border);
    }
    .contacts-toolbar {
      align-items: stretch;
      flex-direction: column;
    }
    .filter-group button {
      flex: 1;
    }
    .tag-filter-bar {
      grid-template-columns: 1fr;
      align-items: stretch;
    }
    .tag-filter-bar > span {
      padding-inline: 2px;
    }
    .clear-tags {
      justify-self: start;
    }

    .contacts-layout {
      flex: none;
    }
    .contact-ledger {
      height: min(56dvh, 560px);
      min-height: 320px;
    }
    .contact-row {
      grid-template-columns: 40px minmax(0, 1fr) 18px;
      padding: 12px;
    }
    .source-mark {
      display: none;
    }
    .dossier-header {
      align-items: flex-start !important;
      flex-direction: column;
    }
    .dialog-actions {
      width: 100%;
    }
    .dialog-actions button:nth-child(2) {
      flex: 1;
    }
    .dossier-grid {
      grid-template-columns: 1fr;
    }
    .dossier-grid > section {
      grid-column: 1 !important;
      border-right: 0;
    }
    .form-grid.three,
    .form-grid.two {
      grid-template-columns: 1fr;
    }
    .span-two {
      grid-column: auto;
    }
    .repeat-row,
    .date-row {
      grid-template-columns: 1fr 44px;
    }
    .repeat-row select,
    .date-row input:nth-child(2),
    .date-row .date-toggle {
      grid-column: 1;
    }
    .repeat-row > button:last-child {
      grid-column: 2;
      grid-row: 1 / span 2;
    }
    .date-row > button:last-child {
      grid-column: 2;
      grid-row: 1 / span 3;
    }
    .toggle-row,
    .tag-entry {
      grid-template-columns: 1fr;
    }

    .danger-footer {
      align-items: stretch;
      flex-direction: column;
    }
    .nested-footer {
      grid-template-columns: 1fr;
    }
  }
  @media (max-height: 720px) {
    .contacts-page {
      height: auto;
      min-height: calc(100dvh - 76px);
      overflow: visible;
    }
    .contacts-layout {
      flex: none;
    }
    .contact-ledger {
      height: min(56dvh, 560px);
      min-height: 320px;
    }
  }
</style>
