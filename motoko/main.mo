import Prim "mo:⛔";
import Cycles "mo:base/ExperimentalCycles";
import Nat64 "mo:base/Nat64";
import Int64 "mo:base/Int64";
import Time "mo:base/Time";

actor {

  // ------------------
  // PageList -- Dummy data for now to fill up heap.
  // -------------------
  type Pages = {#pages: Blob}; // Some multiple of page size.
  type PageList = ?{ pages : Pages; tail : PageList };

  stable var pageList : PageList = null;
  
  public func grow(numPages : Nat) : async () {
      let pageSize = 1 << 16 : Nat32;
      let pages = #pages (Prim.arrayMutToBlob(Prim.Array_init<Nat8>(numPages * (Prim.nat32ToNat(pageSize)), 137 : Nat8)));
      pageList := ?{ pages; tail = pageList };
  };

  // ------------------
  // SnapShots -- Meta data and regions for snapshots.
  // -------------------

  type SnapshotMeta = {
      id : Nat32;
      time : Int64;
  };
  
  type Snapshot = SnapshotMeta and { region : Region };
  type SnapshotInfo = SnapshotMeta and { pages : Nat64 };

  type Snapshots = ?(Snapshot and {tail : Snapshots });

  stable var snapshotCount : Nat = 0;
  stable var snapshots : Snapshots = null;

  func infoOfSnapshot(s : Snapshot) : SnapshotInfo {
      { s with pages = Prim.regionSize(s.region) }
  };
  
  func _getLastSnapshotInfo() : ?SnapshotInfo {
      switch(snapshots) {
      case null null;
      case (?s) ?infoOfSnapshot(s);
      }
  };
  
  type Info = {
      rtsMemorySize : Nat ;
      rtsMemorySizeMb : Float ;
      lastSnapshot : ?SnapshotInfo
  };

  public query func getInfo() : async Info {
      let mb : Float = Prim.intToFloat(Prim.nat32ToNat(1 << 20 : Nat32));
      { rtsMemorySize = Prim.rts_memory_size() ;
        rtsMemorySizeMb = Prim.intToFloat(Prim.rts_memory_size()) / mb;
        lastSnapshot = _getLastSnapshotInfo() ;
      }
  };
  
  public query func getLastSnapshotInfo() : async ?SnapshotInfo {
      _getLastSnapshotInfo()
  };

  public func updateSnapshot() : async SnapshotInfo {
      switch snapshots {
      case null { infoOfSnapshot(doSnapshot_(Prim.regionNew())) };
      case (?s) { infoOfSnapshot(doSnapshot_(s.region)) };
    }
  };

  func doSnapshot_(r : Region) : Snapshot {
      let s : Snapshot = {
          id = Prim.natToNat32(snapshotCount);
          region = r ;
          time = Prim.intToInt64(Time.now());
      };
      snapshots := ?{ s with tail = snapshots };
      snapshotCount += 1;
      Prim.regionMainMemorySnapshot(r);
      s
  };
  
  public func createSnapshot() : async SnapshotInfo {
      infoOfSnapshot(doSnapshot_(Prim.regionNew()))
  };

  // Reads a Blob from the most recent snapshot.
  public query func readLastSnapshot(offset : Nat64, size : Nat) : async Blob {
      switch snapshots {
      case null {
               Prim.trap("No snapshots. Please use updateSnapshot or createSnapshot()");
           };
      case (?s) {
               Prim.regionLoadBlob(s.region, offset, size)
           }
      }
  };

  // ----------------------------------
  // Boilerplate code to accept cycles.
  // todo 20231026 -- Does this work?
  //
  // See: https://internetcomputer.org/docs/current/developer-docs/backend/motoko/simple-cycles
  // ----------------------------------
  //
  stable var cyclesBalance = 0;
  
  // Return the cycles received up to the capacity allowed
  public func wallet_receive() : async { available: Nat64; accepted: Nat64 } {
    let available = Cycles.available();
    let accepted = Cycles.accept(available);
    cyclesBalance += accepted;
    {
        available = Nat64.fromNat(available);
        accepted = Nat64.fromNat(accepted)
      
    };
  };

  public query func getCyclesBalance() : async Nat { cyclesBalance  };  
};
