import { create } from "zustand";
import {
  type Asset,
  type AssetFilters,
  type AssetPagination,
  listAssets,
  getAsset,
} from "../lib/api/businessApi";
import { useAuthStore } from "./auth";

export type AssetsStatus = "idle" | "loading" | "success" | "error";

type AssetsState = {
  assets: Asset[];
  pagination: AssetPagination | null;
  status: AssetsStatus;
  error: string | null;
  detailAsset: Asset | null;
  detailStatus: AssetsStatus;
  detailError: string | null;
  loadAssets: (filters?: AssetFilters) => Promise<void>;
  loadMoreAssets: (filters?: AssetFilters) => Promise<void>;
  loadAssetDetail: (assetId: string) => Promise<void>;
  reset: () => void;
};

export const useAssetsStore = create<AssetsState>((set, get) => ({
  assets: [],
  pagination: null,
  status: "idle",
  error: null,
  detailAsset: null,
  detailStatus: "idle",
  detailError: null,

  loadAssets: async (filters) => {
    const token = useAuthStore.getState().token;
    if (!token) {
      set({ status: "error", error: "no_session" });
      return;
    }

    set({ status: "loading", error: null });

    try {
      const res = await listAssets(token, filters);
      set({ assets: res.assets, pagination: res.pagination, status: "success" });
    } catch (e) {
      set({
        status: "error",
        error: e instanceof Error ? e.message : "Impossible de charger les actifs",
      });
    }
  },

  loadMoreAssets: async (filters) => {
    const token = useAuthStore.getState().token;
    const { pagination, assets } = get();
    if (!token || !pagination) return;

    const nextOffset = pagination.offset + pagination.returned;
    set({ status: "loading" });

    try {
      const res = await listAssets(token, { ...filters, offset: nextOffset });
      set({
        assets: [...assets, ...res.assets],
        pagination: res.pagination,
        status: "success",
      });
    } catch (e) {
      set({
        status: "error",
        error: e instanceof Error ? e.message : "Impossible de charger les actifs",
      });
    }
  },

  loadAssetDetail: async (assetId) => {
    const token = useAuthStore.getState().token;
    if (!token) {
      set({ detailStatus: "error", detailError: "no_session" });
      return;
    }

    set({ detailStatus: "loading", detailError: null, detailAsset: null });

    try {
      const asset = await getAsset(token, assetId);
      set({ detailAsset: asset, detailStatus: "success" });
    } catch (e) {
      set({
        detailStatus: "error",
        detailError:
          e instanceof Error
            ? e.message
            : "Cet actif est introuvable ou inaccessible",
      });
    }
  },

  reset: () =>
    set({
      assets: [],
      pagination: null,
      status: "idle",
      error: null,
      detailAsset: null,
      detailStatus: "idle",
      detailError: null,
    }),
}));
